"""Held-out oracle for the `recovery` case. Not copied into solver
workspaces; grade() imports the solver's `jobrunner` package from their
workspace and runs these tests against it.

The crash tests inject a fault at every boundary of every step, then resume
the job in a *fresh* JobRunner (a new process would see exactly this: the
checkpoint files and nothing else) and assert exactly-once side effects and a
fully recovered context. Plain loops rather than subTest(): the grader counts
one failure per test method.
"""
import json
import tempfile
import unittest
from pathlib import Path

from jobrunner import (AFTER_ACTION, AFTER_COMMIT, AFTER_INTENT, BEFORE_INTENT,
                       BOUNDARIES, CheckpointError, CheckpointStore,
                       InjectedFault, JOB_END, JOB_START, JobError, JobRunner,
                       Step, crash_at, record_boundaries, validate_steps)

_STEP_NAMES = ("alpha", "beta", "gamma")
_EXPECTED_CONTEXT = {"alpha": 1, "beta": 2, "gamma": 3}


class _EffectLog:
    """Stands in for the world the steps mutate: the side effect is an entry
    here, and a step's probe simply asks whether its entry already landed."""

    def __init__(self):
        self.entries = []

    def record(self, name):
        self.entries.append(name)

    def count(self, name):
        return self.entries.count(name)


def _build_steps(effects, failing=None, failures_left=0):
    """Three steps whose probes consult the effect log. `failing` names a step
    whose action raises for its first `failures_left` invocations."""
    budget = {"left": failures_left}

    def make(name, value):
        def action(context):
            if name == failing and budget["left"] > 0:
                budget["left"] -= 1
                raise RuntimeError(f"{name} refused to run")
            effects.record(name)
            return {name: value}

        def probe(context):
            return name in effects.entries

        return Step(name, action, probe)

    return [make(name, value) for name, value in zip(_STEP_NAMES, (1, 2, 3))]


class _JobCase(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.checkpoint_path = self.root / "job.checkpoint"
        self.effects = _EffectLog()

    def _use_fresh_job(self, tag):
        """Point the helpers at an untouched checkpoint and effect log."""
        self.checkpoint_path = self.root / f"job-{tag}.checkpoint"
        self.effects = _EffectLog()

    def new_runner(self, failing=None, failures_left=0):
        """A fresh runner over the same checkpoint: a restarted process."""
        steps = _build_steps(self.effects, failing=failing, failures_left=failures_left)
        return JobRunner("job-1", steps, self.checkpoint_path)

    def assert_exactly_once(self, expected=_STEP_NAMES):
        for name in expected:
            self.assertEqual(self.effects.count(name), 1,
                             f"{name} ran {self.effects.count(name)} time(s): {self.effects.entries}")
        self.assertEqual(len(self.effects.entries), len(expected), self.effects.entries)


class CheckpointStoreTests(_JobCase):
    def test_load_of_a_never_saved_checkpoint_is_none(self):
        self.assertIsNone(CheckpointStore(self.checkpoint_path).load())

    def test_save_then_load_round_trips_and_versions_increase(self):
        store = CheckpointStore(self.checkpoint_path)
        first = store.save({"completed": ["alpha"]})
        second = store.save({"completed": ["alpha", "beta"]})
        self.assertEqual(second["version"], first["version"] + 1)
        self.assertEqual(store.load(), {"completed": ["alpha", "beta"]})

    def test_a_damaged_live_file_is_repaired_from_the_backup(self):
        store = CheckpointStore(self.checkpoint_path)
        store.save({"completed": []})
        store.save({"completed": ["alpha"]})
        self.checkpoint_path.write_text('{"version": 2, "payload": {"comp',
                                        encoding="utf-8")

        self.assertEqual(store.load(), {"completed": []})
        store.backup_path.unlink()
        self.assertEqual(store.load(), {"completed": []}, "load() must repair the live file")

    def test_a_truncated_live_file_without_a_backup_raises(self):
        store = CheckpointStore(self.checkpoint_path)
        store.save({"completed": []})
        self.checkpoint_path.write_text("not json at all", encoding="utf-8")
        with self.assertRaises(CheckpointError):
            store.load()

    def test_an_edited_payload_fails_its_checksum(self):
        store = CheckpointStore(self.checkpoint_path)
        store.save({"completed": ["alpha"]})
        record = json.loads(self.checkpoint_path.read_text(encoding="utf-8"))
        record["payload"]["completed"] = ["alpha", "beta"]
        self.checkpoint_path.write_text(json.dumps(record), encoding="utf-8")
        with self.assertRaises(CheckpointError):
            store.load()

    def test_a_leftover_temp_file_is_discarded_and_never_adopted(self):
        store = CheckpointStore(self.checkpoint_path)
        store.save({"completed": ["alpha"]})
        temp_path = self.checkpoint_path.with_name(self.checkpoint_path.name + ".tmp")
        temp_path.write_text('{"version": 99, "payload": {"completed": ["zzz"]}}',
                             encoding="utf-8")

        self.assertEqual(store.load(), {"completed": ["alpha"]})
        self.assertFalse(temp_path.exists(), "a half-written temp file must be discarded")

    def test_the_live_file_is_never_written_in_place(self):
        store = CheckpointStore(self.checkpoint_path)
        store.save({"completed": []})
        first_inode = self.checkpoint_path.stat().st_ino
        store.save({"completed": ["alpha"]})
        self.assertNotEqual(self.checkpoint_path.stat().st_ino, first_inode)
        self.assertTrue(store.backup_path.exists(), "the previous version must be kept")

    def test_load_returns_a_copy_the_caller_cannot_corrupt(self):
        store = CheckpointStore(self.checkpoint_path)
        store.save({"completed": ["alpha"]})
        loaded = store.load()
        loaded["completed"].append("tampered")
        self.assertEqual(store.load(), {"completed": ["alpha"]})

    def test_the_backup_holds_the_version_before_the_live_one(self):
        store = CheckpointStore(self.checkpoint_path)
        for completed in ([], ["alpha"], ["alpha", "beta"]):
            store.save({"completed": completed})
        live = json.loads(self.checkpoint_path.read_text(encoding="utf-8"))
        backup = json.loads(store.backup_path.read_text(encoding="utf-8"))
        self.assertEqual(live["version"], backup["version"] + 1)
        self.assertEqual(backup["payload"], {"completed": ["alpha"]})

    def test_a_truncated_live_file_falls_back_to_the_previous_version(self):
        store = CheckpointStore(self.checkpoint_path)
        store.save({"completed": []})
        store.save({"completed": ["alpha"]})
        whole = self.checkpoint_path.read_text(encoding="utf-8")
        self.checkpoint_path.write_text(whole[: len(whole) // 2], encoding="utf-8")
        self.assertEqual(store.load(), {"completed": []})

    def test_an_unstorable_payload_raises_and_keeps_the_old_checkpoint(self):
        store = CheckpointStore(self.checkpoint_path)
        store.save({"completed": ["alpha"]})
        with self.assertRaises(CheckpointError):
            store.save({"completed": {"alpha"}})  # a set is not JSON
        self.assertEqual(store.load(), {"completed": ["alpha"]})


class DefinitionValidationTests(_JobCase):
    def test_step_rejects_a_bad_name_or_non_callables(self):
        with self.assertRaises(JobError):
            Step("", lambda context: {}, lambda context: True)
        with self.assertRaises(JobError):
            Step("alpha", None, lambda context: True)
        with self.assertRaises(JobError):
            Step("alpha", lambda context: {}, "not callable")

    def test_validate_steps_rejects_empty_jobs_and_duplicate_names(self):
        step = Step("alpha", lambda context: {}, lambda context: True)
        with self.assertRaises(JobError):
            validate_steps([])
        with self.assertRaises(JobError):
            validate_steps([step, step])

    def test_runner_rejects_an_empty_job_id(self):
        with self.assertRaises(JobError):
            JobRunner("", _build_steps(self.effects), self.checkpoint_path)

    def test_a_checkpoint_from_another_job_is_refused(self):
        CheckpointStore(self.checkpoint_path).save(
            {"job_id": "other-job", "completed": [], "context": {},
             "in_flight": None, "failure": None})
        with self.assertRaises(JobError):
            self.new_runner().run()


class FaultHookTests(_JobCase):
    def test_crash_at_fires_only_for_its_boundary_and_budget(self):
        hook = crash_at(AFTER_ACTION, step="beta", times=1)
        hook(AFTER_ACTION, {"step": "alpha", "index": 0})
        hook(BEFORE_INTENT, {"step": "beta", "index": 1})
        with self.assertRaises(InjectedFault):
            hook(AFTER_ACTION, {"step": "beta", "index": 1})
        hook(AFTER_ACTION, {"step": "beta", "index": 1})  # budget spent

    def test_crash_at_rejects_unknown_boundaries_and_bad_budgets(self):
        with self.assertRaises(JobError):
            crash_at("no_such_boundary")
        with self.assertRaises(JobError):
            crash_at(JOB_START, times=0)


class HappyPathTests(_JobCase):
    def test_every_step_runs_once_in_order(self):
        report = self.new_runner().run()
        self.assertEqual(report.status, "completed")
        self.assertEqual(report.executed, list(_STEP_NAMES))
        self.assertEqual(report.repaired, [])
        self.assertEqual(report.skipped, [])
        self.assertEqual(report.context, _EXPECTED_CONTEXT)
        self.assert_exactly_once()

    def test_running_a_completed_job_again_does_nothing(self):
        self.new_runner().run()
        report = self.new_runner().run()
        self.assertEqual(report.status, "completed")
        self.assertEqual(report.executed, [])
        self.assertEqual(report.skipped, list(_STEP_NAMES))
        self.assert_exactly_once()

    def test_the_final_checkpoint_records_the_whole_job(self):
        self.new_runner().run()
        state = self.new_runner().state()
        self.assertEqual(state["completed"], list(_STEP_NAMES))
        self.assertIsNone(state["in_flight"])
        self.assertEqual(state["context"], _EXPECTED_CONTEXT)

    def test_boundaries_fire_in_the_contracted_order(self):
        log = []
        self.new_runner().run(record_boundaries(log))
        expected = [(JOB_START, None)]
        for name in _STEP_NAMES:
            expected.extend([(BEFORE_INTENT, name), (AFTER_INTENT, name),
                             (AFTER_ACTION, name), (AFTER_COMMIT, name)])
        expected.append((JOB_END, None))
        self.assertEqual(log, expected)

    def test_intent_is_durable_before_the_action_runs(self):
        seen = {}

        def hook(boundary, info):
            if boundary == AFTER_INTENT and info["step"] == "beta":
                seen["state"] = self.new_runner().state()

        self.new_runner().run(hook)
        self.assertEqual(seen["state"]["in_flight"], "beta")
        self.assertEqual(seen["state"]["completed"], ["alpha"])


class CrashAndResumeTests(_JobCase):
    def _crash_then_resume(self, boundary, step):
        hook = crash_at(boundary, step=step, times=1)
        with self.assertRaises(InjectedFault):
            self.new_runner().run(hook)
        report = self.new_runner().run(hook)
        self.assertEqual(report.status, "completed", f"{boundary}/{step}: {report.error}")
        self.assertEqual(report.context, _EXPECTED_CONTEXT, f"{boundary}/{step}")
        self.assert_exactly_once()
        return report

    def test_crash_before_the_intent_checkpoint_reruns_nothing_extra(self):
        report = self._crash_then_resume(BEFORE_INTENT, "beta")
        self.assertEqual(report.skipped, ["alpha"])
        self.assertEqual(report.executed, ["beta", "gamma"])

    def test_crash_after_the_intent_checkpoint_still_runs_the_step_once(self):
        report = self._crash_then_resume(AFTER_INTENT, "beta")
        self.assertEqual(report.executed, ["beta", "gamma"])
        self.assertEqual(report.repaired, [])

    def test_crash_between_the_action_and_its_commit_is_repaired_by_the_probe(self):
        report = self._crash_then_resume(AFTER_ACTION, "beta")
        self.assertEqual(report.repaired, ["beta"], "the probe must prevent a re-run")
        self.assertEqual(report.executed, ["gamma"])

    def test_crash_after_the_commit_skips_the_finished_step(self):
        report = self._crash_then_resume(AFTER_COMMIT, "beta")
        self.assertEqual(report.skipped, ["alpha", "beta"])
        self.assertEqual(report.executed, ["gamma"])

    def test_crash_at_job_start_loses_nothing(self):
        report = self._crash_then_resume(JOB_START, None)
        self.assertEqual(report.executed, list(_STEP_NAMES))

    def test_crash_at_job_end_leaves_a_completed_job(self):
        report = self._crash_then_resume(JOB_END, None)
        self.assertEqual(report.executed, [])
        self.assertEqual(report.skipped, list(_STEP_NAMES))

    def test_every_boundary_of_every_step_recovers_exactly_once(self):
        for boundary in BOUNDARIES:
            for step in _STEP_NAMES:
                self._use_fresh_job(f"{boundary}-{step}")
                self._crash_then_resume(boundary, step if _is_step_boundary(boundary) else None)

    def test_two_crashes_at_the_same_boundary_still_run_each_step_once(self):
        hook = crash_at(AFTER_INTENT, step="beta", times=2)
        with self.assertRaises(InjectedFault):
            self.new_runner().run(hook)
        with self.assertRaises(InjectedFault):
            self.new_runner().run(hook)
        report = self.new_runner().run(hook)
        self.assertEqual(report.status, "completed")
        self.assert_exactly_once()

    def test_a_probe_reporting_a_missing_effect_causes_a_rerun(self):
        steps = [Step("alpha", _always_effect(self.effects), lambda context: False)]
        runner = JobRunner("job-1", steps, self.checkpoint_path)
        with self.assertRaises(InjectedFault):
            runner.run(crash_at(AFTER_ACTION, step="alpha"))
        JobRunner("job-1", steps, self.checkpoint_path).run()
        self.assertEqual(self.effects.count("alpha"), 2,
                         "a probe answering False must make the runner redo the step")

    def test_a_repaired_step_counts_as_repaired_and_not_as_executed(self):
        with self.assertRaises(InjectedFault):
            self.new_runner().run(crash_at(AFTER_ACTION, step="alpha"))
        report = self.new_runner().run()
        self.assertNotIn("alpha", report.executed)
        self.assertEqual(report.repaired, ["alpha"])

    def test_resuming_replays_no_boundary_of_an_already_completed_step(self):
        with self.assertRaises(InjectedFault):
            self.new_runner().run(crash_at(AFTER_COMMIT, step="beta"))
        log = []
        self.new_runner().run(record_boundaries(log))
        self.assertEqual(log, [(JOB_START, None), (BEFORE_INTENT, "gamma"),
                               (AFTER_INTENT, "gamma"), (AFTER_ACTION, "gamma"),
                               (AFTER_COMMIT, "gamma"), (JOB_END, None)])

    def test_only_the_in_flight_step_is_probed(self):
        probed = []
        steps = [Step(name, _recording_action(self.effects, name, value),
                      _recording_probe(self.effects, name, probed))
                 for name, value in zip(_STEP_NAMES, (1, 2, 3))]
        with self.assertRaises(InjectedFault):
            JobRunner("job-1", steps, self.checkpoint_path).run(
                crash_at(AFTER_ACTION, step="beta"))
        JobRunner("job-1", steps, self.checkpoint_path).run()
        self.assertEqual(probed, ["beta"], "only the interrupted step may be probed")
        self.assert_exactly_once()

    def test_a_damaged_checkpoint_mid_job_is_repaired_and_the_job_finishes(self):
        with self.assertRaises(InjectedFault):
            self.new_runner().run(crash_at(AFTER_COMMIT, step="alpha"))
        self.checkpoint_path.write_text('{"version": 3, "payload": {"job', encoding="utf-8")

        report = self.new_runner().run()
        self.assertEqual(report.status, "completed")
        self.assert_exactly_once()

    def test_an_injected_fault_is_never_converted_into_a_report(self):
        with self.assertRaises(InjectedFault):
            self.new_runner().run(crash_at(BEFORE_INTENT, step="alpha"))


class StepFailureTests(_JobCase):
    def test_a_failing_action_stops_the_job_and_is_reported(self):
        report = self.new_runner(failing="beta", failures_left=1).run()
        self.assertEqual(report.status, "failed")
        self.assertEqual(report.failed_step, "beta")
        self.assertIn("refused to run", report.error)
        self.assertEqual(report.executed, ["alpha"])
        self.assertEqual(self.effects.entries, ["alpha"])

    def test_a_failed_job_records_the_failure_and_clears_the_in_flight_mark(self):
        self.new_runner(failing="beta", failures_left=1).run()
        state = self.new_runner().state()
        self.assertIsNone(state["in_flight"])
        self.assertEqual(state["failure"]["step"], "beta")
        self.assertEqual(state["completed"], ["alpha"])

    def test_resuming_after_a_failure_retries_only_the_failed_step(self):
        self.new_runner(failing="beta", failures_left=1).run()
        report = self.new_runner().run()
        self.assertEqual(report.status, "completed")
        self.assertEqual(report.executed, ["beta", "gamma"])
        self.assertEqual(report.skipped, ["alpha"])
        self.assert_exactly_once()
        self.assertEqual(report.context, _EXPECTED_CONTEXT)

    def test_job_end_fires_for_a_failed_run_too(self):
        log = []
        self.new_runner(failing="alpha", failures_left=1).run(record_boundaries(log))
        self.assertEqual(log[-1], (JOB_END, None))

    def test_an_action_returning_a_non_dict_raises(self):
        steps = [Step("alpha", lambda context: "not a dict", lambda context: False)]
        with self.assertRaises(JobError):
            JobRunner("job-1", steps, self.checkpoint_path).run()


def _always_effect(effects):
    def action(context):
        effects.record("alpha")
        return {"alpha": 1}
    return action


def _recording_action(effects, name, value):
    def action(context):
        effects.record(name)
        return {name: value}
    return action


def _recording_probe(effects, name, probed):
    def probe(context):
        probed.append(name)
        return name in effects.entries
    return probe


def _is_step_boundary(boundary):
    """JOB_START/JOB_END carry no step, so a step filter would never match."""
    return boundary not in (JOB_START, JOB_END)


if __name__ == "__main__":
    unittest.main()
