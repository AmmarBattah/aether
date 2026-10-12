"""Deterministic checks for the deliberately failing orchestration fixture."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from orchestration_wait import WORKER


class FailureFixtureTests(unittest.TestCase):
    def run_worker(self, workspace, release):
        (workspace / "worker.py").write_text(WORKER)
        (workspace / "release.json").write_text(json.dumps(release))
        return subprocess.run([sys.executable, "worker.py"], cwd=workspace,
                              capture_output=True, text=True, timeout=5, check=False)

    def test_injected_failure_is_nonzero_and_does_not_manufacture_answer(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            result = self.run_worker(workspace, {"fail": True})
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("RuntimeError: injected worker failure", result.stderr)
            self.assertEqual(json.loads((workspace / "failure.json").read_text()),
                             {"error": "injected worker failure"})
            self.assertFalse((workspace / "answer.json").exists())
            self.assertTrue((workspace / "worker.started").exists())

    def test_normal_completion_is_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            result = self.run_worker(workspace, {"value": 137})
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads((workspace / "answer.json").read_text()), {"answer": 132})
            self.assertFalse((workspace / "failure.json").exists())

    def test_cli_rejects_combined_failure_and_cancellation_before_launch(self):
        result = subprocess.run(
            [sys.executable, str(Path(__file__).with_name("orchestration_wait.py")),
             "--binary", "/nonexistent", "--output-dir", "/nonexistent", "--cancel", "--child-failure"],
            capture_output=True, text=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertIn("separate scenarios", result.stderr)


if __name__ == "__main__":
    unittest.main()
