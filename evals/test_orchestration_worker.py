"""Linux process-identity checks for the cancellation observer."""
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from orchestration_worker import track_worker, worker_exited


def identity(pid):
    return {"pid": pid, "pid_namespace": os.stat("/proc/self/ns/pid").st_ino}


@unittest.skipUnless(hasattr(os, "pidfd_open"), "Linux pidfd required")
class WorkerObserverTests(unittest.TestCase):
    def test_observes_owned_worker_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            process = subprocess.Popen(["sleep", "30"], cwd=workspace)
            try:
                (workspace / "worker.started").write_text(json.dumps(identity(process.pid)))
                with track_worker(workspace) as descriptor:
                    self.assertFalse(worker_exited(descriptor))
                    process.terminate()
                    process.wait(timeout=5)
                    self.assertTrue(worker_exited(descriptor))
                with self.assertRaises(OSError):
                    os.fstat(descriptor)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)

    def test_rejects_a_process_outside_the_fixture_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            (workspace / "worker.started").write_text(json.dumps(identity(os.getpid())))
            with self.assertRaisesRegex(ValueError, "isolated workspace"):
                with track_worker(workspace):
                    self.fail("must not accept the test runner as the fixture worker")

    def test_rejects_invalid_pid_types_and_system_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            for pid in (True, 0, 1, -1, "123", None):
                with self.subTest(pid=pid):
                    (workspace / "worker.started").write_text(json.dumps(identity(pid)))
                    with self.assertRaisesRegex(ValueError, "worker PID"):
                        with track_worker(workspace):
                            self.fail("invalid PID was accepted")

    def test_rejects_an_unknown_namespace(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            (workspace / "worker.started").write_text(json.dumps({"pid": os.getpid(), "pid_namespace": 1}))
            with self.assertRaisesRegex(RuntimeError, "found 0"):
                with track_worker(workspace):
                    self.fail("unknown namespace was accepted")


if __name__ == "__main__":
    unittest.main()
