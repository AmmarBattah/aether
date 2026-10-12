"""Fault-injection boundaries and hook factories. See ../CONTRACT.md."""
from .errors import InjectedFault, JobError

JOB_START = "job_start"
BEFORE_INTENT = "before_intent"
AFTER_INTENT = "after_intent"
AFTER_ACTION = "after_action"
AFTER_COMMIT = "after_commit"
JOB_END = "job_end"
BOUNDARIES = (JOB_START, BEFORE_INTENT, AFTER_INTENT, AFTER_ACTION,
              AFTER_COMMIT, JOB_END)


def crash_at(boundary, step=None, times=1):
    """A hook that raises InjectedFault the first `times` matching calls.

    Matching means the boundary is `boundary` and, when `step` is given, the
    boundary belongs to that step. Every other call returns None.
    """
    remaining = [times]

    def hook(event, info):
        assert event in BOUNDARIES, f"hook called with unknown boundary: {event!r}"
        if event != boundary or remaining[0] < 1:
            return
        if step is not None and info.get("step") != step:
            return
        remaining[0] -= 1
        raise InjectedFault(f"injected fault at {event} (step={info.get('step')!r})")

    return hook


def record_boundaries(log):
    """A hook that appends `(boundary, step_name)` to `log` and never raises."""
    if not isinstance(log, list):
        raise JobError(f"log must be a list, got {type(log).__name__}")

    def hook(event, info):
        assert event in BOUNDARIES, f"hook called with unknown boundary: {event!r}"
        log.append((event, info.get("step")))

    return hook
