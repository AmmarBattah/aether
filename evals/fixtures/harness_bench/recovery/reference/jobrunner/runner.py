"""Checkpointed, resumable, exactly-once job runner. See ../CONTRACT.md."""
from .checkpoint import CheckpointStore
from .errors import CheckpointError, InjectedFault, JobError
from .faults import (AFTER_ACTION, AFTER_COMMIT, AFTER_INTENT, BEFORE_INTENT,
                     JOB_END, JOB_START)
from .steps import validate_steps

_STATE_FIELDS = ("job_id", "completed", "context", "in_flight", "failure")


class JobReport:
    """What one `run()` call did. `completed` is cumulative, the rest is
    about this call only."""

    __slots__ = ("status", "completed", "executed", "repaired", "skipped",
                 "context", "failed_step", "error")

    def __init__(self, status, completed, executed, repaired, skipped, context,
                 failed_step=None, error=None):
        assert status in ("completed", "failed"), f"unknown status: {status!r}"
        assert (failed_step is None) == (status == "completed"), \
            "a failed report names its step and a completed one does not"
        self.status = status
        self.completed = list(completed)
        self.executed = list(executed)
        self.repaired = list(repaired)
        self.skipped = list(skipped)
        self.context = dict(context)
        self.failed_step = failed_step
        self.error = error

    def __repr__(self):
        return (f"JobReport(status={self.status!r}, executed={self.executed!r}, "
                f"repaired={self.repaired!r}, skipped={self.skipped!r})")


class JobRunner:
    """Runs `steps` in order, checkpointing intent before each side effect."""

    def __init__(self, job_id, steps, checkpoint_path, *, checkpoints=None):
        if not isinstance(job_id, str) or not job_id:
            raise JobError(f"job_id must be a non-empty string, got {job_id!r}")
        self._job_id = job_id
        self._steps = validate_steps(steps)
        self._checkpoints = checkpoints if checkpoints is not None else CheckpointStore(checkpoint_path)

    @property
    def job_id(self):
        return self._job_id

    def state(self):
        """The durable state, loaded fresh (repairing a damaged checkpoint)."""
        return self._load_state()

    def run(self, fault_hook=None):
        """Run every step not yet completed, exactly once each, and report."""
        hook = _require_hook(fault_hook)
        state = self._load_state()
        progress = {"executed": [], "repaired": [], "skipped": []}
        hook(JOB_START, {"step": None, "index": None, "resumed": _is_resumed(state)})
        for index, step in enumerate(self._steps):
            if step.name in state["completed"]:
                progress["skipped"].append(step.name)
                continue
            failure = self._process_step(step, index, state, hook, progress)
            if failure is not None:
                hook(JOB_END, {"step": None, "index": None, "status": "failed"})
                return _report("failed", state, progress, failure)
        hook(JOB_END, {"step": None, "index": None, "status": "completed"})
        assert len(state["completed"]) == len(self._steps), "a completed job ran every step"
        return _report("completed", state, progress, None)

    def _process_step(self, step, index, state, hook, progress):
        """Run one step; returns a failure dict, or None when it completed."""
        info = {"step": step.name, "index": index}
        resuming = state["in_flight"] == step.name
        hook(BEFORE_INTENT, info)
        self._record_intent(state, step.name)
        hook(AFTER_INTENT, info)
        if resuming and _probe(step, state["context"]):
            progress["repaired"].append(step.name)
        else:
            failure = self._run_action(step, state, progress)
            if failure is not None:
                return failure
            self._record_result(state, step.name)
            hook(AFTER_ACTION, info)
        self._record_completion(state, step.name)
        hook(AFTER_COMMIT, info)
        return None

    def _run_action(self, step, state, progress):
        try:
            updates = step.action(state["context"])
        except InjectedFault:
            raise
        except Exception as exc:  # an ordinary step failure, recorded not hidden
            self._record_failure(state, step.name, exc)
            return {"step": step.name, "error": f"{type(exc).__name__}: {exc}"}
        if not isinstance(updates, dict):
            raise JobError(f"step {step.name!r} returned {updates!r}, expected a dict")
        progress["executed"].append(step.name)
        state["context"].update(updates)
        return None

    def _record_intent(self, state, step_name):
        state["in_flight"] = step_name
        state["failure"] = None
        self._checkpoints.save(state)

    def _record_result(self, state, step_name):
        """Make the action's context updates durable while the step is still
        in flight, so a probe-repaired resume does not lose them."""
        assert state["in_flight"] == step_name, "the acting step must be the in-flight one"
        assert state["failure"] is None, "a running step has no recorded failure"
        self._checkpoints.save(state)

    def _record_completion(self, state, step_name):
        assert state["in_flight"] == step_name, "a completing step must be the in-flight one"
        state["completed"].append(step_name)
        state["in_flight"] = None
        self._checkpoints.save(state)

    def _record_failure(self, state, step_name, exc):
        state["in_flight"] = None
        state["failure"] = {"step": step_name, "error": f"{type(exc).__name__}: {exc}"}
        self._checkpoints.save(state)

    def _load_state(self):
        stored = self._checkpoints.load()
        if stored is None:
            return {"job_id": self._job_id, "completed": [], "context": {},
                    "in_flight": None, "failure": None}
        _require_state_shape(stored)
        if stored["job_id"] != self._job_id:
            raise JobError(f"checkpoint belongs to job {stored['job_id']!r}, not {self._job_id!r}")
        _require_known_steps(stored, {step.name for step in self._steps})
        return stored


def _report(status, state, progress, failure):
    return JobReport(status, state["completed"], progress["executed"],
                     progress["repaired"], progress["skipped"], state["context"],
                     failed_step=None if failure is None else failure["step"],
                     error=None if failure is None else failure["error"])


def _probe(step, context):
    answer = step.probe(context)
    if not isinstance(answer, bool):
        raise JobError(f"step {step.name!r} probe returned {answer!r}, expected a bool")
    return answer


def _require_hook(fault_hook):
    if fault_hook is None:
        return lambda boundary, info: None
    if not callable(fault_hook):
        raise JobError(f"fault_hook must be callable, got {fault_hook!r}")
    return fault_hook


def _is_resumed(state):
    return bool(state["completed"]) or state["in_flight"] is not None


def _require_state_shape(state):
    missing = [field for field in _STATE_FIELDS if field not in state]
    if missing:
        raise CheckpointError(f"checkpoint is missing {missing}")
    if not isinstance(state["completed"], list) or not isinstance(state["context"], dict):
        raise CheckpointError("checkpoint completed/context fields have the wrong type")


def _require_known_steps(state, known_names):
    unknown = [name for name in state["completed"] if name not in known_names]
    if unknown:
        raise JobError(f"checkpoint names steps this job does not define: {unknown}")
