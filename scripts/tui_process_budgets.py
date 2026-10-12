#!/usr/bin/env python3
"""Linux process measurements of the real TUI and its restart/replay path."""

import argparse
import contextlib
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "clients/python"))
from aether.client import AetherClient  # noqa: E402
from aether import protocol  # noqa: E402

VOLATILE = Path("/dev/shm")
KEPT_STATE = ("config", "data")


class Fixture:
    def __init__(self, binary):
        self.root = Path(os.path.relpath(tempfile.mkdtemp(prefix="pb-", dir="target")))
        self.binary = str(Path(binary).resolve())
        # What the daemon and CLI create is memory-backed: a stalled build disk held
        # daemon startup past the readiness deadline. close() keeps config/ and data/.
        assert VOLATILE.is_dir(), f"process budget fixture needs a memory-backed {VOLATILE}"
        self.state = Path(tempfile.mkdtemp(prefix="aether-pb-", dir=VOLATILE))
        self.socket = str(self.state / "s")
        assert len(os.fsencode(self.socket)) < 104, self.socket
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("AETHER_")}
        for name in KEPT_STATE:
            (self.state / name).mkdir()
        # The empty CLI creates a waiting root only when a model is configured.
        # Never depend on the developer's installed engines or credentials.
        # This fixture sends no objective, so its loopback endpoint is not used.
        (self.state / "config/config.toml").write_text('[model]\ndefault = "idle-fixture/idle"\n')
        (self.state / "config/providers.toml").write_text('''[providers.idle-fixture]
name = "Idle process fixture"
protocol = "openai-chat-completions"
base_url = "http://127.0.0.1:9/v1"
[providers.idle-fixture.auth]
none = true
[[providers.idle-fixture.models]]
id = "idle"
name = "Idle fixture (never invoked)"
''')
        self.env.update(AETHER_CONFIG_DIR=str(self.state / "config"),
                        AETHER_DATA_DIR=str(self.state / "data"),
                        AETHER_SOCKET=self.socket, TERM="xterm-256color")
        self.children = []
        self.stopped_groups = set()
        self.drain_errors = []
        self.terminals = []
        self.daemon = None
        self.generation = 0
        print(f"process budget artifacts: {self.root.resolve()}", flush=True)

    def spawn(self, args, name, terminal=False, extra_env=None):
        env = dict(self.env, **(extra_env or {}))
        if not terminal:
            with (self.root / name).open("wb") as output:
                child = subprocess.Popen(args, env=env, stdout=output, stderr=output, start_new_session=True)
        else:
            master, slave = pty.openpty()
            try:
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 200, 160, 0, 0))
                child = subprocess.Popen(args, env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
            except BaseException:
                os.close(master)
                raise
            finally:
                os.close(slave)
            self.children.append(child)
            reader = threading.Thread(target=self.drain, args=(master, name), daemon=True)
            try:
                reader.start()
            except RuntimeError:
                os.close(master)
                raise
            self.terminals.append((master, reader))
        if child not in self.children:
            self.children.append(child)
        return child

    def drain(self, descriptor, name):
        try:
            with (self.root / name).open("wb") as output:
                while True:
                    try:
                        chunk = os.read(descriptor, 65536)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            return  # Linux PTY has no remaining slave.
                        raise
                    if not chunk:
                        return
                    output.write(chunk)
                    output.flush()
        except OSError as error:
            self.drain_errors.append(error)

    def connect(self):
        stream = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            stream.settimeout(2)
            stream.connect(self.socket)
            return AetherClient(stream)
        except BaseException:
            stream.close()
            raise

    def start_daemon(self):
        self.generation += 1
        name = f"daemon-{self.generation}.log"
        self.daemon = self.spawn([self.binary, "daemon", "--foreground"], name)
        deadline = time.monotonic() + 15
        last_error = None
        while time.monotonic() < deadline:
            self.alive(self.daemon)
            try:
                with self.connect() as client:
                    status = client.request(protocol.status(), timeout=2)
                    ready = time.monotonic()
                return ready, status
            except (OSError, TimeoutError, ConnectionError) as error:
                last_error = error
                time.sleep(.01)
        raise AssertionError(f"daemon readiness timeout: {last_error}; see {self.root / name}")

    def alive(self, child):
        assert child.poll() is None, f"child {child.pid} exited {child.returncode}; logs in {self.root}"

    def pause(self, child):
        child.send_signal(signal.SIGSTOP)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            self.alive(child)
            state = Path(f"/proc/{child.pid}/stat").read_text().rsplit(")", 1)[1].split()[0]
            if state == "T":
                return
            time.sleep(.005)
        raise AssertionError(f"child {child.pid} did not stop")

    def stop(self, child):
        if child not in self.stopped_groups:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass  # The owned session may already have exited completely.
            self.stopped_groups.add(child)
        child.wait(timeout=5)

    def keep_state(self):
        try:
            for name in KEPT_STATE:
                shutil.copytree(self.state / name, self.root / name)
            shutil.rmtree(self.state)
        except OSError as error:
            return [f"fixture state left in {self.state}, not kept in {self.root}: {error}"]
        kept = all((self.root / name).is_dir() for name in KEPT_STATE)
        assert kept and not self.state.exists(), f"fixture state not moved to {self.root}"
        return []

    def close(self):
        original_error = sys.exception()
        with deferred_controller_signals() as pending:
            errors = []
            for child in reversed(self.children):
                try:
                    self.stop(child)
                except (OSError, subprocess.TimeoutExpired) as error:
                    errors.append(str(error))
            for descriptor, reader in self.terminals:
                reader.join(timeout=5)
                os.close(descriptor)
                if reader.is_alive():
                    errors.append("PTY drainer did not stop")
            errors.extend(str(error) for error in self.drain_errors)
            errors.extend(self.keep_state())
        if errors:
            failure = RuntimeError(f"fixture cleanup failed: {errors}")
            if original_error is None:
                if pending:
                    failure.add_note(f"controller signals during cleanup: {pending}")
                raise failure
            original_error.add_note(str(failure))
        if pending:
            cancellation = controller_interrupted(pending[0])
            if original_error is not None:
                original_error.add_note(str(cancellation))
            else:
                raise cancellation


def parse_cpu_ticks(stat):
    fields = stat.rsplit(")", 1)[1].split()
    user, system = int(fields[11]), int(fields[12])
    if user < 0 or system < 0:
        raise ValueError("negative process CPU counter")
    return user + system


def cpu_ticks(pid):
    return parse_cpu_ticks(Path(f"/proc/{pid}/stat").read_text())


def validate_replay(expected, observed):
    actual = [r["event"] for r in observed if r["kind"] == "event"]
    if actual != expected:
        raise AssertionError(f"replay gaps/duplicates/content mismatch: expected {expected}; actual {actual}")
    last = next(r for r in reversed(observed) if r["kind"] == "event")
    if last["cursor"] != expected[-1]["seq"]:
        raise AssertionError("reducer cursor did not reach the last delivered event")


def assert_quiescent_root(status):
    data = status["data"]
    assert data["agents_live"] == data["agents_waiting"] == 1, "CLI did not settle at its waiting root"
    for field in ("agents_running", "agents_blocked", "tasks_active", "engine_turns_running"):
        assert data[field] == 0, f"non-idle fixture: {field}={data[field]}"


def await_waiting_root(fixture, child, seconds=15):
    deadline = time.monotonic() + seconds
    while True:
        fixture.alive(child)
        with fixture.connect() as client:
            status = client.request(protocol.status(), timeout=2)
        if status["data"]["agents_waiting"] == 1:
            return status
        assert time.monotonic() < deadline, f"CLI created no waiting root in {seconds}s: {status['data']}"
        time.sleep(.05)


def idle_measurements(fixture):
    _, status = fixture.start_daemon()
    assert status["data"]["agents_live"] == status["data"]["agents_running"] == 0, status
    (fixture.root / "idle-status.json").write_text(json.dumps(status, indent=2))
    child = fixture.spawn([fixture.binary], "idle.ansi", terminal=True)
    # A loaded host can take longer than the settling time to create the root.
    await_waiting_root(fixture, child)
    time.sleep(4)
    fixture.alive(child)
    assert (fixture.root / "idle.ansi").stat().st_size > 100, "TUI never rendered"
    with fixture.connect() as client:
        status = client.request(protocol.status(), timeout=2)
    (fixture.root / "idle-settled-status.json").write_text(json.dumps(status, indent=2))
    assert_quiescent_root(status)
    settled_seq = status["data"]["last_seq"]
    hz = os.sysconf("SC_CLK_TCK")
    assert hz > 0, "invalid CPU accounting resolution"
    samples = []
    for sample in range(3):
        before = cpu_ticks(child.pid)
        start = time.monotonic()
        time.sleep(5)
        fixture.alive(child)
        after = cpu_ticks(child.pid)
        wall = time.monotonic() - start
        assert after >= before and wall >= 5, "invalid CPU sample"
        measured = dict(sample=sample, ticks=after-before, hz=hz, wall_seconds=wall,
                        cpu_percent=100*(after-before)/hz/wall,
                        tick_resolution_percent=100/hz/wall)
        samples.append(measured)
        print(f"idle CPU: {json.dumps(measured)}", flush=True)
    with fixture.connect() as client:
        status = client.request(protocol.status(), timeout=2)
    assert_quiescent_root(status)
    assert status["data"]["last_seq"] == settled_seq, "durable activity occurred while sampling idle CPU"
    (fixture.root / "idle-final-status.json").write_text(json.dumps(status, indent=2))
    fixture.stop(child)
    fixture.stop(fixture.daemon)
    return samples


def records(path):
    if not path.exists():
        return []
    lines = path.read_text().splitlines(keepends=True)
    return [json.loads(line) for line in lines if line.endswith("\n")]


def until(fixture, child, receipt, predicate, description, seconds=5):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        fixture.alive(child)
        observed = records(receipt)
        if predicate(observed):
            return time.monotonic(), observed
        time.sleep(.005)
    raise AssertionError(f"{description} timed out; last receipts: {records(receipt)[-3:]}; {fixture.root}")


def create_events(fixture, prefix, count):
    with fixture.connect() as client:
        for index in range(count):
            client.request(protocol.create_task(objective=f"{prefix}-{index}"), timeout=2)
        events = client.request(protocol.get_events(after=0, limit=1000, query={}), timeout=2)["data"]
    assert len(events) < 1000, "fixture event query was truncated"
    objectives = [e["kind"]["objective"] for e in events if e["kind"]["type"] == "task.created"
                  and e["kind"]["objective"].startswith(prefix + "-")]
    assert objectives == [f"{prefix}-{index}" for index in range(count)], "fixture did not persist every task event"
    return events


def replay_measurement(fixture, runner_test, outage):
    fixture.start_daemon()
    receipt = fixture.root / "runner.jsonl"
    child = fixture.spawn([runner_test, "--exact", "runner::process_tests::process_budget_child",
                           "--ignored", "--nocapture"], "runner.ansi", terminal=True,
                          extra_env={"AETHER_BUDGET_SOCKET": fixture.socket,
                                     "AETHER_BUDGET_RECEIPT": str(receipt.resolve())})
    _, initial = until(fixture, child, receipt,
                       lambda rows: any(r["kind"] == "subscribed" for r in rows), "initial subscription")
    cursor = next(r["cursor"] for r in initial if r["kind"] == "subscribed")
    expected = create_events(fixture, "live", 3)
    until(fixture, child, receipt, lambda rows: any(r["cursor"] == expected[-1]["seq"] for r in rows),
          "live events consumed")
    # A stopped daemon leaves a real Status request pending at the next 2s tick.
    fixture.pause(fixture.daemon)
    time.sleep(2.3)
    crash = time.monotonic()
    fixture.stop(fixture.daemon)
    until(fixture, child, receipt, lambda rows: any(r["kind"] == "failed_reply" for r in rows),
          "pending request failed")
    until(fixture, child, receipt, lambda rows: any(r["kind"] == "disconnected" for r in rows),
          "disconnect consumed")
    time.sleep(outage)
    # Hold the UI only while seeding missed durable events; that cost is included in ready-to-replay.
    fixture.pause(child)
    ready, _ = fixture.start_daemon()
    expected = create_events(fixture, "missed", 3)
    child.send_signal(signal.SIGCONT)
    completed, observed = until(fixture, child, receipt,
                               lambda rows: any(r["kind"] == "event" and r["cursor"] == expected[-1]["seq"]
                                                for r in rows), "restart replay consumed")
    expected = [e for e in expected if e["seq"] > cursor]
    (fixture.root / "expected-replay.json").write_text(json.dumps(expected, indent=2))
    validate_replay(expected, observed)
    late = create_events(fixture, "late", 2)
    _, observed = until(fixture, child, receipt,
                        lambda rows: any(r["cursor"] == late[-1]["seq"] for r in rows), "late live events")
    time.sleep(1.1)
    fixture.alive(child)
    observed = records(receipt)
    late_expected = [e for e in late if e["seq"] > cursor]
    validate_replay(late_expected, observed)
    assert sum(r["kind"] == "reconnected" for r in observed) == 1, "extra reconnect after late events"
    assert sum(r["kind"] == "subscribed" for r in observed) == 2, "extra or missing subscription"
    assert observed[-1]["connected"], "late disconnect left the runner offline"
    measured = dict(outage_seconds=outage, crash_to_ready_seconds=ready-crash,
                    ready_to_replay_seconds=completed-ready, crash_to_replay_seconds=completed-crash,
                    replay_events=len(expected), final_events=len(late_expected))
    print(f"restart replay: {json.dumps(measured)}", flush=True)
    (fixture.root / "measurements.json").write_text(json.dumps(measured, indent=2))
    return measured


class ControllerDeadline(RuntimeError):
    pass


class ControllerCancelled(RuntimeError):
    pass


def controller_interrupted(signum):
    if signum == signal.SIGALRM:
        return ControllerDeadline("process budget deadline (90s)")
    return ControllerCancelled("process budget controller received SIGTERM")


def deadline_expired(signum, _frame):
    raise controller_interrupted(signum)


def controller_cancelled(signum, _frame):
    raise controller_interrupted(signum)


@contextlib.contextmanager
def deferred_controller_signals():
    pending = []
    def record(signum, _frame):
        if signum not in pending:
            pending.append(signum)
    previous = {sig: signal.signal(sig, record) for sig in (signal.SIGTERM, signal.SIGALRM)}
    try:
        yield pending
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


@contextlib.contextmanager
def controller_limits():
    previous_alarm = signal.signal(signal.SIGALRM, deadline_expired)
    previous_term = signal.signal(signal.SIGTERM, controller_cancelled)
    signal.alarm(90)
    try:
        yield
    finally:
        signal.alarm(0)
        signal.signal(signal.SIGALRM, previous_alarm)
        signal.signal(signal.SIGTERM, previous_term)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runner-test", required=True)
    parser.add_argument("--binary", default="target/release/aether")
    parser.add_argument("--enforce-idle-cpu", action="store_true",
                        help="fail on any sampled CPU ticks; a known gap, not a no-timers proof")
    args = parser.parse_args()
    os.chdir(ROOT)
    with controller_limits():
        run_measurements(args)


def run_measurements(args):
    results = {"idle": [], "reconnect": []}
    with contextlib.closing(Fixture(args.binary)) as fixture:
        results["idle"] = idle_measurements(fixture)
        (fixture.root / "measurements.json").write_text(json.dumps(results, indent=2))
    for outage in (.15, 3.25, 5.25):
        with contextlib.closing(Fixture(args.binary)) as fixture:
            results["reconnect"].append(replay_measurement(fixture, args.runner_test, outage))
            (fixture.root / "measurements.json").write_text(json.dumps(results, indent=2))
    print("OPEN GAP: idle CPU is measured, NOT budget-enforced. Periodic housekeeping "
          "still violates no-timers; zero observed ticks only means below accounting resolution.", flush=True)
    if args.enforce_idle_cpu:
        assert all(s["ticks"] == 0 for s in results["idle"]), "sampled idle CPU budget violated; the idle-CPU gap remains open"
    assert all(s["ready_to_replay_seconds"] < 1 for s in results["reconnect"]), "under-1s restart/replay budget violated"


if __name__ == "__main__":
    main()
