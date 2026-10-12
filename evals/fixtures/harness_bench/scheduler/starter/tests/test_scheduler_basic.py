"""Visible smoke tests. These only cover the easy happy path; the graded
oracle (outside this workspace) checks the harder contract requirements."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from scheduler import Clock, SchedulerError, Scheduler, Task


class SchedulerBasicTests(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.scheduler = Scheduler(self.clock)
        self.calls = []

    def _record(self, name):
        def action():
            self.calls.append(name)
        return action

    def test_linear_chain_runs_in_order(self):
        self.scheduler.add_task(Task("a", action=self._record("a")))
        self.scheduler.add_task(Task("b", deps=["a"], action=self._record("b")))
        self.scheduler.add_task(Task("c", deps=["b"], action=self._record("c")))
        report = self.scheduler.run()
        self.assertEqual(report.order, ["a", "b", "c"])
        self.assertTrue(all(r.status == "success" for r in report.results.values()))

    def test_unknown_dependency_raises_before_running(self):
        self.scheduler.add_task(Task("a", deps=["missing"], action=self._record("a")))
        with self.assertRaises(SchedulerError):
            self.scheduler.run()
        self.assertEqual(self.calls, [])

    def test_task_retries_until_success(self):
        attempts = {"n": 0}

        def flaky():
            attempts["n"] += 1
            if attempts["n"] < 2:
                raise RuntimeError("not yet")

        self.scheduler.add_task(Task("a", action=flaky, max_attempts=3, retry_backoff=1.0))
        report = self.scheduler.run()
        self.assertEqual(report.results["a"].status, "success")
        self.assertEqual(report.results["a"].attempts, 2)

    def test_deadline_already_passed_skips_without_running_action(self):
        self.scheduler.add_task(Task("a", action=self._record("a"), deadline=-1))
        report = self.scheduler.run()
        self.assertEqual(report.results["a"].status, "skipped")
        self.assertEqual(self.calls, [])

    def test_failed_task_skips_direct_dependent(self):
        def boom():
            raise RuntimeError("boom")

        self.scheduler.add_task(Task("a", action=boom))
        self.scheduler.add_task(Task("b", deps=["a"], action=self._record("b")))
        report = self.scheduler.run()
        self.assertEqual(report.results["a"].status, "failed")
        self.assertEqual(report.results["b"].status, "skipped")


if __name__ == "__main__":
    unittest.main()
