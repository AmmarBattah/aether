"""Offline subprocess ownership, timeout, and cleanup regression tests."""
import os
import shutil
import tempfile
import time
import unittest
from unittest.mock import Mock, patch
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import harness_bench as hb
from harness_bench_test_support import TEST_SCRATCH, node


class SubprocessTimeoutTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-subproc-")
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def _paths(self):
        return os.path.join(self.tmp, "out.log"), os.path.join(self.tmp, "err.log")

    def test_fast_success_is_recorded_without_timeout(self):
        stdout_path, stderr_path = self._paths()
        info = hb.run_subprocess_capturing(["true"], self.tmp, timeout=5, stdout_path=stdout_path, stderr_path=stderr_path)
        self.assertEqual(info["exit_code"], 0)
        self.assertFalse(info["timed_out"])
        self.assertIsNone(info["cleanup_error"])

    def test_nonzero_exit_is_recorded_as_failure_not_timeout(self):
        stdout_path, stderr_path = self._paths()
        info = hb.run_subprocess_capturing(["false"], self.tmp, timeout=5, stdout_path=stdout_path, stderr_path=stderr_path)
        self.assertEqual(info["exit_code"], 1)
        self.assertFalse(info["timed_out"])

    def test_slow_process_is_killed_and_marked_timed_out(self):
        stdout_path, stderr_path = self._paths()
        started = time.monotonic()
        info = hb.run_subprocess_capturing(["sleep", "30"], self.tmp, timeout=0.3,
                                            stdout_path=stdout_path, stderr_path=stderr_path)
        elapsed = time.monotonic() - started
        self.assertTrue(info["timed_out"])
        self.assertIsNone(info["exit_code"])
        self.assertIsNone(info["cleanup_error"], "the process group must actually be gone after cleanup")
        self.assertLess(elapsed, 10, "SIGTERM/SIGKILL escalation must not wait out the full sleep")

    def test_env_argument_is_applied_to_the_child(self):
        stdout_path, stderr_path = self._paths()
        env = {"PATH": os.environ["PATH"], "HB_MARKER": "present"}
        hb.run_subprocess_capturing(["sh", "-c", "printf %s \"$HB_MARKER\""], self.tmp,
                                     timeout=5, stdout_path=stdout_path, stderr_path=stderr_path, env=env)
        self.assertEqual(Path(stdout_path).read_text(), "present")


class LifecycleRegressionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix='lifecycle-'))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_exited_leader_does_not_leave_background_child(self):
        marker = self.tmp / 'survivor'
        ready = self.tmp / 'ready'
        child = ("import signal,time; from pathlib import Path; "
                 "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                 f"Path({str(ready)!r}).touch(); time.sleep(0.5); Path({str(marker)!r}).touch(); time.sleep(30)")
        leader = ("import subprocess,sys,time; from pathlib import Path; "
                  f"subprocess.Popen([sys.executable,'-c',{child!r}]); "
                  f"\nwhile not Path({str(ready)!r}).exists(): time.sleep(0.01)")
        result = hb.run_subprocess_capturing([sys.executable, '-c', leader], str(self.tmp), 5,
                                            str(self.tmp / 'out'), str(self.tmp / 'err'))
        self.assertEqual(result['exit_code'], 0)
        time.sleep(0.7)
        self.assertFalse(marker.exists(), 'background child survived group cleanup')

    def test_startup_failure_still_stops_and_trial_is_graded(self):
        case = {'id': 'fake', 'objective': 'do it'}
        def prepare(_case, workspace):
            workspace.mkdir()
            (workspace / 'solution.py').write_text('x=1')
            return {}
        graded = ({'passed': 1, 'total': 1, 'errors': []}, None, None, 'no second grade')
        with patch.object(hb, 'prepare_workspace', side_effect=prepare), \
             patch.object(hb, 'run_grade_with_tests', return_value=graded) as grade, \
             patch.object(hb, 'start_daemon', side_effect=RuntimeError('partial startup')), \
             patch.object(hb, 'stop_daemon', return_value=None) as stop:
            result = hb.run_trial('/fake', 'aether-single-codex', case, 1, 1, '/bin/true', str(self.tmp))
        self.assertEqual(stop.call_count, 1)
        self.assertEqual(grade.call_count, 1)
        self.assertIn('partial startup', result['workflow_error'])
        self.assertFalse(result['success'])
        self.assertEqual(result['grade']['passed'], 1)
        self.assertEqual((result['grade_error'], result['grade_by_test'], result['grade_by_test_error']),
                         (None, None, 'no second grade'))
        self.assertTrue(Path(result['trial_dir'], 'result.json').is_file())

    def test_timeout_cancels_and_stops_before_usage_capture(self):
        order = []
        client = Mock()
        client.get_agent.return_value = {'run': 'run'}
        tree = {'nodes': {'root': node('root', 'running')}}
        def request(command, timeout=5):
            if command['cmd'] == 'get_tree':
                return {'data': tree}
            order.append('cancel')
            return {'data': None}
        client.request.side_effect = request
        result = {}
        with patch.object(hb, 'start_daemon', return_value=Mock()), \
             patch.object(hb, 'AetherClient', return_value=client), \
             patch.object(hb, '_spawn_root', return_value=('root', None)), \
             patch.object(hb, '_wait_root', return_value=('timeout', 'elapsed', None)), \
             patch.object(hb, 'wait_for_tree_quiescent', return_value=(tree, False)), \
             patch.object(hb, 'stop_daemon', side_effect=lambda *a: order.append('stop')), \
             patch.object(hb, 'capture_stopped_run', side_effect=lambda *a: order.append('capture')):
            hb.run_aether_trial('/fake', hb.CONDITIONS['aether-single-codex'], 'task', self.tmp,
                                1, str(self.tmp), '/bin/true', result)
        self.assertEqual(order, ['cancel', 'stop', 'capture'])
        self.assertEqual(result['workflow_status'], 'timeout')
        self.assertIn('solve_seconds', result)

    def test_real_startup_failure_is_bounded_and_process_owned(self):
        env = hb.isolated_env(str(self.tmp))
        with self.assertRaisesRegex(RuntimeError, 'daemon exited'):
            hb.start_daemon('/bin/false', env, str(self.tmp))
        self.assertTrue((self.tmp / 'daemon-stderr.log').exists())

    def test_native_exception_does_not_skip_grade(self):
        def prepare(_case, workspace):
            workspace.mkdir()
            return {}
        graded = ({'passed': 0, 'total': 1, 'errors': []}, None, None, 'no second grade')
        with patch.object(hb, 'prepare_workspace', side_effect=prepare), \
             patch.object(hb, 'run_native_trial', side_effect=OSError('exec failed')), \
             patch.object(hb, 'run_grade_with_tests', return_value=graded):
            result = hb.run_trial('/fake', 'native-codex', {'id': 'fake', 'objective': 'x'}, 1,
                                  1, '/bin/true', str(self.tmp))
        self.assertEqual(result['grade']['total'], 1)
        self.assertIn('exec failed', result['harness_error'])

    def test_a_failed_test_by_test_grade_is_recorded_and_changes_nothing_that_is_counted(self):
        def prepare(_case, workspace):
            workspace.mkdir()
            return {}
        def solve(_condition, _task, _workspace, _timeout, _trial_dir, result):
            result['workflow_status'] = 'completed'
        outcomes = {}
        for by_test, by_test_error in ((None, 'grade_by_test() raised OSError: cannot fork'),
                                       ({'passed': 1, 'failed': 0, 'hung': 0, 'not_run': 0, 'total': 1}, None)):
            graded = ({'passed': 1, 'total': 1, 'errors': []}, by_test, None, by_test_error)
            with patch.object(hb, 'prepare_workspace', side_effect=prepare), \
                 patch.object(hb, 'run_native_trial', side_effect=solve), \
                 patch.object(hb, 'run_grade_with_tests', return_value=graded):
                result = hb.run_trial('/fake', 'native-codex', {'id': 'fake', 'objective': 'x'}, 1,
                                      1, '/bin/true', str(self.tmp))
            self.assertEqual((result['grade_by_test'], result['grade_by_test_error']), (by_test, by_test_error))
            outcomes[by_test_error] = (result['grade'], result['grade_error'], result['success'], result['status'])
        self.assertEqual(len(set(map(str, outcomes.values()))), 1)
        self.assertEqual(outcomes[None], ({'passed': 1, 'total': 1, 'errors': []}, None, True, 'completed'))


if __name__ == "__main__":
    unittest.main()
