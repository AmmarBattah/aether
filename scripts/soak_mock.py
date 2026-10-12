#!/usr/bin/env python3
"""Exercise an isolated daemon with live agents, mid-turn messages and local tools.

The default duration is 24 hours. Short runs validate this driver, not the soak
acceptance requirement. Only the in-process mock model receives requests.
"""
import argparse
import hashlib
import json
import math
import os
import re
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
from http.server import ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "clients/python"))
from aether import AetherClient, protocol
from mock_openai import Handler


class ModelReceipts:
    def __init__(self):
        self.pending = set()
        self.seen = 0
        self.lock = threading.Lock()

    def expect(self, marker):
        with self.lock:
            self.pending.add(marker)

    def observe(self, request):
        markers = set()
        for message in request.get("messages", []):
            if message.get("role") == "user":
                markers.update(re.findall(r"soak-steer-\d+-\d+", json.dumps(message.get("content"))))
        with self.lock:
            matched = self.pending.intersection(markers)
            self.pending.difference_update(matched)
            self.seen += len(matched)

    def remaining(self):
        with self.lock:
            return len(self.pending)


class DelayedHandler(Handler):
    def read_request(self):
        request = super().read_request()
        self.server.model_receipts.observe(request)
        return request

    def do_POST(self):
        time.sleep(0.2)
        super().do_POST()

    def log_message(self, *_):
        # Metrics and daemon errors are retained; per-request HTTP access logs
        # would dominate the artifacts during a day-long run.
        return


class Receipts:
    def __init__(self):
        self.pending = set()
        self.items = {}
        self.consumed = 0
        self.cursor = 0
        self.tools = 0

    def observe(self, events):
        for event in events:
            self.cursor = max(self.cursor, event["seq"])
            kind = event["kind"]
            if kind["type"] == "inbox.delivered":
                item = kind["item"]
                if item["kind"]["kind"] == "message":
                    self.items[item["id"]] = item["kind"]["message_id"]
            elif kind["type"] == "inbox.consumed":
                for item in kind["items"]:
                    message = self.items.pop(item, None)
                    if message in self.pending:
                        self.pending.remove(message)
                        self.consumed += 1
            elif kind["type"] == "tool.succeeded":
                if kind["outcome"]["is_error"]:
                    raise RuntimeError(f"tool error at event {event['seq']}: {kind}")
                self.tools += 1
            elif kind["type"] in ("agent.failed", "task.failed", "tool.failed", "tool.denied", "tool.cancelled"):
                raise RuntimeError(f"failure at event {event['seq']}: {kind}")


def rss_kib(pid):
    result = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)],
                            capture_output=True, text=True, check=True, timeout=5)
    return int(result.stdout.strip())


def request(client, command):
    return client.request(command)["data"]


def await_condition(check, process, timeout=60):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"daemon exited with {process.returncode}")
        value = check()
        if value:
            return value
        time.sleep(0.02)
    raise TimeoutError(f"condition not satisfied within {timeout}s")


def run(args, output):
    config = output / "config"
    data = output / "data"
    workspace = output / "workspace"
    for path in (config, data, workspace):
        path.mkdir()
    (workspace / "README.md").write_text("# Soak fixture\n", encoding="utf-8")
    env = dict(os.environ, AETHER_CONFIG_DIR=str(config), AETHER_DATA_DIR=str(data),
               AETHER_SOCKET=str(output / "daemon.sock"))
    binary = str(Path(args.binary).resolve())
    client = None
    process = None
    receipts = None
    server = None
    thread = None
    report = {"requested_seconds": args.seconds, "agents": args.agents, "status": "running"}
    started = time.monotonic()
    try:
        with open(binary, "rb") as executable:
            report["binary_sha256"] = hashlib.file_digest(executable, "sha256").hexdigest()
        server = ThreadingHTTPServer(("127.0.0.1", 0), DelayedHandler)
        server.model_receipts = ModelReceipts()
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        (config / "providers.toml").write_text(f'''
    [providers.mock]
    name = "Isolated soak mock"
    protocol = "openai-chat-completions"
    base_url = "http://127.0.0.1:{server.server_port}/v1"
    [providers.mock.auth]
    none = true
    [[providers.mock.models]]
    id = "mock-1"
    name = "Mock"
    [providers.mock.models.capabilities]
    context_window = 32768
    max_output_tokens = 4096
    tools = true
    ''', encoding="utf-8")
        (config / "config.toml").write_text('[model]\ndefault="mock/mock-1"\n[scheduler]\nmax_concurrent_turns=32\n', encoding="utf-8")
        thread.start()
        with (output / "daemon.log").open("w") as log:
            process = subprocess.Popen([binary, "daemon", "--foreground"], cwd=workspace,
                                       env=env, stdout=log, stderr=subprocess.STDOUT,
                                       start_new_session=True)
        await_condition(lambda: Path(env["AETHER_SOCKET"]).exists(), process)
        client = AetherClient.connect_unix(env["AETHER_SOCKET"])
        agents = [request(client, protocol.create_agent(
            role="soak", start=True, model={"provider": "mock", "model": "mock-1"},
            config={"long_lived": True, "approval": {"require_for": []}}))["id"]
            for _ in range(args.agents)]
        receipts = Receipts()
        steering = 0
        rounds = 0
        first_rss = None
        peak_rss = 0

        def settled():
            while True:
                events = request(client, protocol.get_events(after=receipts.cursor, limit=1000,
                    query={"types": ["inbox.delivered", "inbox.consumed", "agent.failed", "task.failed", "tool.failed", "tool.succeeded", "tool.denied", "tool.cancelled"]}))
                receipts.observe(events)
                if len(events) < 1000:
                    break
            rows = [request(client, protocol.get_agent(id=agent)) for agent in agents]
            return not receipts.pending and server.model_receipts.remaining() == 0 and all(row["status"]["state"] == "waiting" for row in rows)

        await_condition(settled, process)
        started = time.monotonic()
        with (output / "samples.jsonl").open("w") as metrics:
            while time.monotonic() - started < args.seconds:
                for agent in agents:
                    receipts.pending.add(request(client, protocol.send_message(
                        to=agent, content="please list the files in the workspace"))["message"])
                for index, agent in enumerate(agents):
                    marker = f"soak-steer-{rounds}-{index}"
                    server.model_receipts.expect(marker)
                    row = request(client, protocol.get_agent(id=agent))
                    if row["status"]["state"] == "running":
                        steering += 1
                    receipts.pending.add(request(client, protocol.send_message(
                        to=agent, content=f"hello steering round {rounds} {marker}"))["message"])
                await_condition(settled, process)
                status = request(client, protocol.status())
                if status["resource_leases"]:
                    raise RuntimeError("resource leases remain after all agents settled")
                sample = {"elapsed_seconds": time.monotonic() - started,
                          "rss_kib": rss_kib(process.pid), "consumed": receipts.consumed,
                          "steered_while_running": steering, "last_seq": status["last_seq"]}
                rounds += 1
                if first_rss is None:
                    first_rss = sample["rss_kib"]
                peak_rss = max(peak_rss, sample["rss_kib"])
                metrics.write(json.dumps(sample) + "\n")
                metrics.flush()
                delay = min(args.interval, max(0, args.seconds - sample["elapsed_seconds"]))
                time.sleep(delay)
        if not rounds or steering == 0 or receipts.tools == 0:
            raise RuntimeError("run must exercise mid-turn messages and successful tools")
        growth = peak_rss - first_rss
        if growth > args.max_rss_growth_mib * 1024:
            raise RuntimeError(f"RSS growth {growth} KiB exceeded the configured limit")
        report.update(status="passed", elapsed_seconds=time.monotonic() - started,
                      rounds=rounds, model_received=server.model_receipts.seen, successful_tools=receipts.tools, consumed=receipts.consumed, steering=steering,
                      rss_growth_kib=growth,
                      full_duration=args.seconds >= 86400 and time.monotonic() - started >= 86400 and args.agents >= 20)
    except BaseException as error:
        report.update(status="failed", error=str(error), elapsed_seconds=time.monotonic() - started)
        if receipts is not None:
            report.update(pending_message_ids=sorted(receipts.pending),
                          last_observed_event=receipts.cursor,
                          consumed=receipts.consumed, successful_tools=receipts.tools)
        if server is not None and hasattr(server, "model_receipts"):
            report.update(pending_model_markers=server.model_receipts.remaining(),
                          model_received=server.model_receipts.seen)
        raise
    finally:
        cleanup_errors = []
        def cleanup(action):
            try:
                action()
            except Exception as error:
                cleanup_errors.append(str(error))

        if client is not None:
            cleanup(client.close)
        if process is not None:
            cleanup(lambda: stop_daemon(process))
        if server is not None:
            if thread is not None and thread.is_alive():
                cleanup(server.shutdown)
                cleanup(lambda: thread.join(timeout=5))
                if thread.is_alive():
                    cleanup_errors.append("mock server thread did not stop")
            cleanup(server.server_close)
        if cleanup_errors:
            report.update(status="failed", cleanup_errors=cleanup_errors)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        if cleanup_errors and sys.exc_info()[0] is None:
            raise RuntimeError("cleanup failed: " + "; ".join(cleanup_errors))


def stop_daemon(process):
    if process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        # The owned process can exit between poll and signal.
        process.wait(timeout=10)
        return
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary")
    parser.add_argument("--seconds", type=float, default=86400)
    parser.add_argument("--agents", type=int, default=20)
    parser.add_argument("--interval", type=float, default=10)
    parser.add_argument("--max-rss-growth-mib", type=float, default=128)
    args = parser.parse_args()
    if not all(math.isfinite(value) for value in (args.seconds, args.interval, args.max_rss_growth_mib)) or args.seconds <= 0 or not 1 <= args.agents <= 200 or args.interval < 0 or args.max_rss_growth_mib < 0:
        parser.error("positive duration, 1..200 agents, and nonnegative interval/memory limit required")
    output = Path(tempfile.mkdtemp(prefix="aether-soak-"))
    print(f"artifacts: {output}", flush=True)
    run(args, output)


if __name__ == "__main__":
    main()
