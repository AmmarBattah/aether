"""Capture startup failures must expose the private daemon's diagnostic."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class ParallelCaptureTests(unittest.TestCase):
    def run_lane(self, fail=False):
        scratch = ROOT / "target" / "capture-parallel-tests"
        scratch.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        scripts = root / "scripts"
        scripts.mkdir()
        capture = scripts / "tui_capture.sh"
        capture.write_text(f"#!{sys.executable}\n" + """
import os, pathlib, sys, time
out = pathlib.Path(sys.argv[2])
out.mkdir(parents=True)
group = ('mode' if os.environ.get('MODE_LANE') == '1' else
         'width' if os.environ.get('SMOKE') == '1' else 'theme')
(out.parent / (group + '-' + out.name + '.started')).touch()
deadline = time.monotonic() + 5
while group != 'mode' and len(list(out.parent.glob(group + '-*.started'))) != 4:
    if time.monotonic() >= deadline:
        sys.exit('four captures did not start together')
    time.sleep(0.01)
if os.environ.get('FAIL_CAPTURE') == '1' and out.name == 'auto':
    (out / 'failed').touch()
    sys.exit(17)
time.sleep(0.1)
(out / 'finished').touch()
""")
        capture.chmod(0o700)
        (scripts / "check_captures.py").write_text("""
import pathlib, sys
out = pathlib.Path(sys.argv[1])
assert (out / 'finished').exists(), 'checker ran before capture finished'
with (out.parent / 'checks').open('a') as stream:
    stream.write(out.name + '\\n')
""")
        ci = (ROOT / "scripts" / "ci.sh").read_text()
        wait_start = ci.index("wait_for_captures() {")
        wait_end = ci.index("\n}\n", wait_start) + 3
        lane_start = ci.index('  echo "== lane: wide')
        lane_end = ci.index("  captures_passed=1", lane_start)
        result = subprocess.run(
            ["bash", "-c", "set -euo pipefail\n" + ci[wait_start:wait_end] + ci[lane_start:lane_end]],
            cwd=root, env=dict(os.environ, CAPTURES=str(root / "captures"),
                               AETHER_CI_BINARY="fixture", FAIL_CAPTURE=str(int(fail))),
            capture_output=True, text=True, timeout=15,
        )
        return result, root / "captures"

    def test_all_sessions_run_and_all_existing_checkers_follow(self):
        result, captures = self.run_lane()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(list(captures.glob("theme-*.started"))), 4)
        self.assertEqual(len(list(captures.glob("width-*.started"))), 4)
        self.assertEqual(len(list(captures.glob("mode-*.started"))), 2)
        self.assertEqual((captures / "checks").read_text().splitlines(), [
            "dark", "auto", "light", "mono", "width-59", "width-60", "width-99", "width-100",
            "mode-80", "mode-120",
        ])

    def test_failed_session_waits_for_other_children_and_fails_lane(self):
        result, captures = self.run_lane(fail=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("TUI capture session", result.stderr)
        self.assertTrue((captures / "auto" / "failed").exists())
        for theme in ["dark", "light", "mono"]:
            self.assertTrue((captures / theme / "finished").exists())
        self.assertEqual(len(list(captures.glob("*.started"))), 4)
        self.assertFalse((captures / "checks").exists())


class StartupTests(unittest.TestCase):
    def test_daemon_failure_is_reported_before_the_tui_starts(self):
        # The daemon socket lives here, and a runner's TMPDIR can be too deep for one.
        with tempfile.TemporaryDirectory(prefix="capture-test-", dir="/tmp") as directory:
            root = Path(directory)
            binary = root / "fake-aether"
            binary.write_text("""#!/usr/bin/env python3
import sys
if sys.argv[1:] == ['daemon', '--foreground']:
    print('fixture daemon failure: cannot open storage', file=sys.stderr)
    sys.exit(17)
if sys.argv[1:] == ['status']:
    sys.exit(1)
print('unexpected capture command: ' + ' '.join(sys.argv[1:]), file=sys.stderr)
sys.exit(99)
""", encoding="utf-8")
            binary.chmod(0o700)
            output = root / "captures"
            result = subprocess.run(
                ["bash", "scripts/tui_capture.sh", str(binary), str(output)],
                cwd=ROOT, env=dict(os.environ, TMPDIR=directory),
                capture_output=True, text=True, timeout=15,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("test daemon exited before readiness", result.stderr)
            self.assertIn("fixture daemon failure: cannot open storage", result.stderr)
            self.assertNotIn("unexpected capture command", result.stderr)
            self.assertEqual(list(output.glob("*.txt")), [])
            self.assertIn("fixture daemon failure", (output / "failure" / "aetherd.startup.log").read_text())


# A stand-in for aether: a ready daemon, and a TUI with the mode lane's screens
# that quits on Ctrl+C and closes its terminal before it ends.
FAKE_AETHER = """#!/usr/bin/env python3
import os, select, signal, sys, time, tty
if sys.argv[1:] == ['status']:
    sys.exit(0)
if sys.argv[1:] == ['daemon', '--foreground']:
    signal.pause()
if sys.argv[1:2] != ['tui']:
    sys.exit('unexpected capture command: ' + ' '.join(sys.argv[1:]))
HOME = 'Message assistant | I am a mock model | workspace has | Ran ls\\r\\ncolours: %s %s' % (
    os.environ.get('COLORTERM'), os.environ.get('TERM'))
def show(screen):
    os.write(1, ('\\x1b[2J\\x1b[H' + screen + '\\r\\n').encode())
tty.setraw(0)
show(HOME)
typed = b''
while True:
    select.select([0], [], [])
    # Seconds without the CPU: keys sent meanwhile are read together, as one.
    time.sleep(STARVED)
    keys = os.read(0, 64)
    if not keys:
        sys.exit('the terminal closed before Ctrl+C')
    typed = (typed + keys)[-8:]
    if typed.endswith(b'/mode\\r'):
        show('View modes')
    elif keys == b'\\x1b':
        show(HOME)
    elif keys == b'\\x03':
        break
show('Agents keep running in the daemon. Reattach with: aether')
signal.signal(signal.SIGHUP, signal.SIG_IGN)
for descriptor in (0, 1, 2):
    os.close(descriptor)
time.sleep(GAP)
ENDING
"""


@unittest.skipUnless(shutil.which("tmux"), "needs tmux")
class SessionTests(unittest.TestCase):
    def run_mode_lane(self, ending, environment=None, starved=0, gap=1):
        # The daemon socket lives here, and a runner's TMPDIR can be too deep for one.
        temporary = tempfile.TemporaryDirectory(prefix="capture-test-", dir="/tmp")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        binary = root / "fake-aether"
        fake = FAKE_AETHER.replace("ENDING", ending).replace("STARVED", str(starved))
        fake = fake.replace("GAP", str(gap))
        binary.write_text(fake, encoding="utf-8")
        binary.chmod(0o700)
        output = root / "captures"
        result = subprocess.run(
            ["bash", "scripts/tui_capture.sh", str(binary), str(output), "dark", "80"],
            cwd=ROOT, env=dict(environment or os.environ, TMPDIR=temporary.name, MODE_LANE="1"),
            capture_output=True, text=True, timeout=120,
        )
        return result, output

    def test_exit_status_that_follows_the_closed_terminal_is_waited_for(self):
        result, output = self.run_mode_lane("os._exit(0)")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Agents keep running in the daemon", (output / "15-quit.txt").read_text())
        self.assertIn("Pane is dead (status 0", (output / "15-quit.txt").read_text())
        self.assertFalse((output / "failure").exists())

    def test_exit_status_that_tmux_missed_is_asked_for(self):
        # Before 3.6 a tmux built with utempter loses an exit this soon after the terminal closed.
        result, output = self.run_mode_lane("os._exit(0)", gap=0.0003)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Pane is dead (status 0", (output / "15-quit.txt").read_text())

    def test_colour_depth_does_not_come_from_the_callers_environment(self):
        # Before 3.6 tmux sets no COLORTERM in a pane, and a hosted runner's shell has none.
        bare = {key: value for key, value in os.environ.items() if key != "COLORTERM"}
        result, output = self.run_mode_lane("os._exit(0)", bare)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("colours: truecolor xterm-256color", (output / "01-startup.txt").read_text())

    def test_a_panel_closed_with_escape_is_gone_before_the_next_key(self):
        # Escape and the key after it, read together, are Alt+key: nothing closes or quits.
        result, output = self.run_mode_lane("os._exit(0)", starved=0.3)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Agents keep running in the daemon", (output / "15-quit.txt").read_text())

    def test_failed_quit_names_the_exit_status(self):
        result, output = self.run_mode_lane("os._exit(3)")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("aether quit with status 3\n", result.stderr)
        self.assertIn("status=3", (output / "failure" / "pane-state.txt").read_text())

    def test_killed_quit_names_the_signal(self):
        result, output = self.run_mode_lane("os.kill(os.getpid(), signal.SIGKILL)")
        self.assertNotEqual(result.returncode, 0)
        self.assertRegex(result.stderr, r"aether quit with signal 9\n")
        self.assertRegex((output / "failure" / "pane-state.txt").read_text(), r"status= signal=9\n")


if __name__ == "__main__":
    unittest.main()
