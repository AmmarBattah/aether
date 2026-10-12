"""Visible smoke tests. These only cover the easy happy path; the graded
oracle (outside this workspace) checks the harder contract requirements."""
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from jobrunner import (BEFORE_INTENT, CheckpointStore, InjectedFault, JOB_END,
                       JOB_START, JobError, JobRunner, Step, crash_at,
                       record_boundaries, validate_steps)


def _steps(effects):
    def make(name, value):
        def action(context):
            effects.append(name)
            return {name: value}

        return Step(name, action, lambda context: name in effects)

    return [make("alpha", 1), make("beta", 2)]


class JobRunnerBasicTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.path = Path(self._tmp.name) / "job.checkpoint"
        self.effects = []

    def runner(self, failing=None):
        steps = _steps(self.effects)
        if failing is not None:
            steps = [step for step in steps if step.name != failing] + [
                Step(failing, _raising_action(failing), lambda context: False)]
        return JobRunner("job-1", steps, self.path)

    def test_a_clean_run_executes_every_step_once(self):
        report = self.runner().run()
        self.assertEqual(report.status, "completed")
        self.assertEqual(report.executed, ["alpha", "beta"])
        self.assertEqual(report.context, {"alpha": 1, "beta": 2})
        self.assertEqual(self.effects, ["alpha", "beta"])

    def test_running_a_finished_job_again_changes_nothing(self):
        self.runner().run()
        report = self.runner().run()
        self.assertEqual(report.executed, [])
        self.assertEqual(report.skipped, ["alpha", "beta"])
        self.assertEqual(self.effects, ["alpha", "beta"])

    def test_the_job_start_and_job_end_boundaries_bracket_the_run(self):
        log = []
        self.runner().run(record_boundaries(log))
        self.assertEqual(log[0], (JOB_START, None))
        self.assertEqual(log[-1], (JOB_END, None))
        self.assertIn((BEFORE_INTENT, "alpha"), log)

    def test_a_crash_before_the_first_side_effect_loses_nothing(self):
        hook = crash_at(BEFORE_INTENT, step="alpha")
        with self.assertRaises(InjectedFault):
            self.runner().run(hook)
        self.assertEqual(self.effects, [])
        report = self.runner().run(hook)
        self.assertEqual(report.status, "completed")
        self.assertEqual(self.effects, ["alpha", "beta"])

    def test_a_failing_step_stops_the_job_and_is_reported(self):
        report = self.runner(failing="beta").run()
        self.assertEqual(report.status, "failed")
        self.assertEqual(report.failed_step, "beta")
        self.assertEqual(report.executed, ["alpha"])

    def test_checkpoints_round_trip_and_version_upwards(self):
        store = CheckpointStore(self.path)
        self.assertIsNone(store.load())
        first = store.save({"completed": []})
        second = store.save({"completed": ["alpha"]})
        self.assertEqual(second["version"], first["version"] + 1)
        self.assertEqual(store.load(), {"completed": ["alpha"]})

    def test_a_checkpoint_from_another_job_is_refused(self):
        self.runner().run()
        with self.assertRaises(JobError):
            JobRunner("other-job", _steps(self.effects), self.path).run()

    def test_invalid_definitions_raise_job_error(self):
        with self.assertRaises(JobError):
            Step("", lambda context: {}, lambda context: True)
        with self.assertRaises(JobError):
            validate_steps([])
        with self.assertRaises(JobError):
            JobRunner("", _steps(self.effects), self.path)


def _raising_action(name):
    def action(context):
        raise RuntimeError(f"{name} refused to run")

    return action


if __name__ == "__main__":
    unittest.main()
