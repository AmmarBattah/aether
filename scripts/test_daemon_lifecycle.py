"""Exercise smoke-fixture startup failures and cleanup without a model or daemon."""
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parent.parent


class DaemonLifecycleTests(unittest.TestCase):
    def test_smoke_failure_prints_the_daemon_diagnostic(self):
        with tempfile.TemporaryDirectory(prefix='aether-fixture-test-') as directory:
            binary = Path(directory) / 'fake-aether'
            binary.write_text('''#!/usr/bin/env python3
import os, pathlib, sys
if sys.argv[1] == 'daemon':
    log = pathlib.Path(os.environ['AETHER_DATA_DIR']) / 'logs' / 'aetherd.log.fixture'
    log.parent.mkdir(parents=True, exist_ok=True)
    log.write_text('fixture startup failure: cannot initialize storage\\n')
    if '--foreground' in sys.argv:
        print(log.read_text(), file=sys.stderr)
        sys.exit(17)
    sys.exit(0)
sys.exit(1)
''')
            binary.chmod(0o700)
            result = subprocess.run(
                ['bash', 'scripts/e2e_mock.sh', str(binary)], cwd=ROOT,
                env=dict(os.environ, TMPDIR=directory), capture_output=True, text=True, timeout=25,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("exited before readiness", result.stderr)
            self.assertIn('fixture startup failure: cannot initialize storage', result.stdout + result.stderr)

    def test_delayed_and_never_ready_children_are_reaped(self):
        # deadline: wait_for_test_daemon's budget. cap: subprocess.run timeout,
        # needs deadline + stop_test_child's 5s bounded reap + slack.
        deadlines = {'delayed': 10, 'never': 2}
        caps = {'delayed': 20, 'never': 12}
        for mode in ['delayed', 'never']:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory(prefix='aether-child-test-') as directory:
                root = Path(directory)
                binary = root / 'fake-aether'
                binary.write_text('''#!/usr/bin/env python3
import os, pathlib, sys, time
root = pathlib.Path(os.environ['FIXTURE_ROOT'])
if sys.argv[1] == 'status':
    sys.exit(0 if (root / 'ready').exists() else 1)
(root / 'pid').write_text(str(os.getpid()))
if os.environ['FIXTURE_MODE'] == 'delayed':
    time.sleep(0.2)
    (root / 'ready').touch()
while True:
    time.sleep(1)
''')
                binary.chmod(0o700)
                # never's deadline stays short: it burns it in full by design,
                # and integer-SECONDS truncation (lib.sh's wait_for_test_daemon)
                # plus per-poll fork cost eat real budget, not the 0.2s fixture sleep.
                script = '''
. scripts/lib.sh
"$1" daemon --foreground > "$2/daemon.log" 2>&1 &
pid=$!
trap 'stop_test_child "$pid"' EXIT
wait_for_test_daemon "$1" "$pid" "$2/daemon.log" %d
''' % deadlines[mode]
                reaped = False
                try:
                    start = time.monotonic()
                    result = subprocess.run(
                        ['bash', '-c', script, 'fixture', str(binary), directory], cwd=ROOT,
                        env=dict(os.environ, FIXTURE_ROOT=directory, FIXTURE_MODE=mode),
                        capture_output=True, text=True, timeout=caps[mode],
                    )
                    wall = time.monotonic() - start
                    self.assertEqual(result.returncode, 0 if mode == 'delayed' else 1, result.stderr)
                    if mode == "never":
                        self.assertIn("readiness timed out", result.stderr)
                    if mode == "delayed":
                        # Regression guard: readiness detection should be near-instant,
                        # not silently degrade toward the 10s deadline.
                        self.assertLess(wall, 3, f"delayed readiness took {wall:.2f}s, expected well under deadline")
                    pid = int((root / 'pid').read_text())
                    with self.assertRaises(ProcessLookupError):
                        os.kill(pid, 0)
                    reaped = True
                finally:
                    if not reaped and (root / 'pid').exists():
                        try:
                            os.kill(int((root / 'pid').read_text()), 9)
                        except ProcessLookupError:
                            pass


if __name__ == '__main__':
    unittest.main()
