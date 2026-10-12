#!/usr/bin/env python3
"""PTY regression for normal, panic, exit signals and job-control restoration."""
import fcntl
import os
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time


def check(executable, mode):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    before = termios.tcgetattr(slave)
    child = None
    output = bytearray()

    def read_output():
        queries = output.count(b"\x1b[6n")
        output.extend(os.read(master, 65536))
        for _ in range(output.count(b"\x1b[6n") - queries):
            os.write(master, b"\x1b[1;1R")

    try:
        def job_control_group():
            os.setpgid(0, 0)
            signal.signal(signal.SIGTSTP, signal.SIG_DFL)

        child = subprocess.Popen(
            [executable, "--exact", "runner::tests::terminal_restore_child", "--nocapture"],
            env=dict(os.environ, AETHER_TERMINAL_RESTORE_TEST=mode, TERM="xterm-256color"),
            stdin=slave, stdout=slave, stderr=slave,
            start_new_session=mode != "SUSPEND",
            preexec_fn=job_control_group if mode == "SUSPEND" else None,
        )
        deadline = time.monotonic() + 10
        while b"RESTORE_READY" not in output:
            assert time.monotonic() < deadline, (mode, bytes(output))
            assert child.poll() is None, (mode, child.returncode, bytes(output))
            if select.select([master], [], [], 0.05)[0]:
                read_output()
        if mode == "SUSPEND":
            while True:
                pid, status = os.waitpid(child.pid, os.WNOHANG | os.WUNTRACED)
                if pid:
                    assert os.WIFSTOPPED(status), (mode, status, bytes(output))
                    assert os.WSTOPSIG(status) == signal.SIGTSTP, status
                    break
                assert time.monotonic() < deadline, (mode, "did not stop", bytes(output))
                if select.select([master], [], [], 0.05)[0]:
                    read_output()
            assert termios.tcgetattr(slave) == before, "suspension must leave cooked mode"
            child.send_signal(signal.SIGCONT)
            while b"RESUME_READY" not in output:
                assert time.monotonic() < deadline, (mode, bytes(output))
                assert child.poll() is None, (mode, child.returncode, bytes(output))
                if select.select([master], [], [], 0.05)[0]:
                    read_output()
            assert termios.tcgetattr(slave) != before, "resume must re-enter raw mode"
            os.write(master, b"x")
        elif mode in ("TERM", "HUP"):
            assert termios.tcgetattr(slave) != before, "fixture must enter raw mode"
            child.send_signal(signal.SIGTERM if mode == "TERM" else signal.SIGHUP)
        else:
            assert termios.tcgetattr(slave) != before, "fixture must enter raw mode"
            os.write(master, b"x")
        while child.poll() is None:
            assert time.monotonic() < deadline, (mode, bytes(output))
            if select.select([master], [], [], 0.05)[0]:
                read_output()
        while select.select([master], [], [], 0.05)[0]:
            read_output()
        assert child.returncode == (101 if mode == "panic" else 0), (mode, child.returncode, bytes(output))
        assert termios.tcgetattr(slave) == before, (mode, "stty differs after exit")
        sequences = [b"\x1b[?2026l", b"\x1b[?1000l", b"\x1b[?1002l", b"\x1b[?1003l",
                     b"\x1b[?1006l", b"\x1b[?2004l", b"\x1b[<1u", b"\x1b[?1049l", b"\x1b[?25h"]
        ready = output.index(b"RESTORE_READY")
        restored = output[ready:]
        for sequence in sequences:
            assert sequence in restored, (mode, sequence, bytes(output))
        restores = 2 if mode == "SUSPEND" else 1
        assert restored.count(b"\x1b[<1u") == restores, (mode, "keyboard stack unbalanced")
        assert restored.count(b"\x1b[?1049l") == restores, (mode, "restoration was not idempotent")
        assert restored.index(b"\x1b[?2026l") < restored.index(b"\x1b[?1049l"), mode
        print(f"terminal restoration {mode}: PASS", flush=True)
    finally:
        if child is not None and child.poll() is None:
            child.kill()
            child.wait(timeout=5)
        os.close(slave)
        os.close(master)


if __name__ == "__main__":
    for mode in sys.argv[2:] or ("normal", "panic", "TERM", "HUP"):
        check(sys.argv[1], mode)
