"""Validate the concurrency grader against deliberately broken references."""
import shutil
import tempfile
import unittest
from pathlib import Path

import harness_bench_cases as hb


REFERENCE = Path(__file__).parent / "fixtures/harness_bench/concurrent/reference/txstore"


class ConcurrencyMutationTests(unittest.TestCase):
    def grade_mutant(self, old, new):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            shutil.copytree(REFERENCE, workspace / "txstore",
                            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
            engine = workspace / "txstore/engine.py"
            source = engine.read_text()
            self.assertEqual(source.count(old), 1, "mutation must target exactly one site")
            engine.write_text(source.replace(old, new))
            return hb.grade("concurrent", workspace, timeout_seconds=20)

    def assert_rejected(self, report, test_name):
        self.assertEqual(report["total"], 49)
        self.assertLess(report["passed"], 49)
        self.assertTrue(any(test_name in error for error in report["errors"]), report)

    def test_holding_state_lock_during_append_is_rejected(self):
        report = self.grade_mutant(
            "with self.store._commit_lock:",
            "with self.store._commit_lock, self.store._state_lock:",
        )
        self.assert_rejected(report, "test_reads_and_begin_progress_during_append")

    def test_missing_commit_serialization_is_rejected(self):
        report = self.grade_mutant(
            "self._commit_lock = RLock()",
            "self._commit_lock = __import__('contextlib').nullcontext()",
        )
        self.assert_rejected(report, "test_disjoint_commits_get_distinct_revisions_and_replay")

    def test_allowing_callback_nested_commit_is_rejected(self):
        report = self.grade_mutant(
            "if self.store._committing:",
            "if False:",
        )
        self.assert_rejected(report, "test_reentrant_commit_is_rejected_and_remains_retryable")

    def test_publishing_before_durable_append_is_rejected(self):
        report = self.grade_mutant(
            "        if self.store.journal is not None:\n"
            "            self.store.journal.append(record)\n"
            "        with self.store._state_lock:\n"
            "            self.store._publish(receipt)",
            "        with self.store._state_lock:\n"
            "            self.store._publish(receipt)\n"
            "        if self.store.journal is not None:\n"
            "            self.store.journal.append(record)",
        )
        self.assert_rejected(report, "test_reads_and_begin_progress_during_append")

    def test_reference_repeatedly_passes_the_isolated_grader(self):
        for _ in range(5):
            report = hb.grade("concurrent", REFERENCE.parent, timeout_seconds=20)
            self.assertEqual(report, {"passed": 49, "total": 49, "errors": []})


if __name__ == "__main__":
    unittest.main()
