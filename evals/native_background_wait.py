#!/usr/bin/env python3
"""Paid, isolated regression: native jobs survive MCP wait without polling."""
import argparse
import json
import os
from pathlib import Path
import tempfile
import time

import harness_bench as hb
from aether_client import init_git_workspace
from orchestration_wait import WORKER, observe_park, parent_counters


def run(binary, output, cancel=False, monitor=False):
    trial = Path(tempfile.mkdtemp(prefix="nb-", dir=hb.SCRATCH_ROOT))
    workspace = trial / "workspace"
    workspace.mkdir()
    (workspace / "worker.py").write_text(WORKER)
    init_git_workspace(str(workspace))
    env = hb.isolated_env(str(trial))
    hb.strip_live_routing_env(env, keep=hb._AETHER_ISOLATED_ENV_KEYS)
    result = {"trial": str(trial), "binary_sha256": hb.sha256_file(binary),
              "cancel": cancel, "monitor": monitor, "errors": []}
    proc = client = root = run_id = None
    try:
        proc = hb.start_daemon(binary, env, str(workspace))
        client = hb.AetherClient(env["AETHER_SOCKET"])
        client.subscribe()
        prompt = (
            "This is an isolated lifecycle integration test. Run "
            + ("python3 worker.py > worker.log " if monitor else "python3 worker.py ")
            + "exactly once "
            "using native Bash with run_in_background=true. Do not detach or use nohup/setsid. "
            "The observer owns release.json; do not create or change it or worker.py. "
            + ("Then start a native Monitor with command tail -F worker.log 2>/dev/null "
               "and timeout_ms=180000. " if monitor else "")
            + "Immediately call Aether wait_for(timeout_seconds=120). Do not poll, sleep, "
            "read interim logs or start another command while waiting. On a completion "
            "notification, read answer.json once. "
            + ("Stop your Monitor with TaskStop. " if monitor else "")
            + "Then call complete_task with summary ANSWER=<the value>. Work only in this fixture."
        )
        agent = client.create_agent(objective=prompt, model="claude-code/sonnet",
                                    effort="low", long_lived=False)
        root, run_id = agent["id"], agent["run"]
        deadline = time.monotonic() + 180
        while True:
            frame = client.next_frame(timeout=max(.01, deadline - time.monotonic()))
            if frame.get("frame") != "event":
                continue
            kind = frame["event"]["kind"]
            if kind.get("name") == "engine.native_wait_started":
                result["wait_started"] = frame["event"]["seq"]
                break
            if time.monotonic() >= deadline:
                raise TimeoutError("native wait never started")
        observe_park(client, 2)
        before = parent_counters(client.get_agent(root))
        observe_park(client, 25)
        after = parent_counters(client.get_agent(root))
        result["wait_observation"] = {"seconds": 25, "before": before, "after": after}
        if before != after:
            result["errors"].append("model work occurred during the unresolved native wait")
        pid = json.loads((workspace / "worker.started").read_text())["pid"]
        os.kill(pid, 0)
        result["worker_alive_after_park_grace"] = True
        if cancel:
            error = hb._cancel_best_effort(client, root, 5)
            if error:
                raise RuntimeError(error)
            deadline = time.monotonic() + 15
            while Path(f"/proc/{pid}").exists() and time.monotonic() < deadline:
                time.sleep(.1)
            result["worker_exited"] = not Path(f"/proc/{pid}").exists()
            if not result["worker_exited"]:
                result["errors"].append("cancel left the worker alive")
            # Cancellation is admitted before the engine's usage/reap finishes.
            _, error, _ = hb.wait_for_run_terminal(client, root, 15)
            if error and client.get_agent(root)["status"]["state"] != "cancelled":
                raise RuntimeError(error)
        else:
            (workspace / "release.json").write_text(json.dumps({"value": 17}))
            released = time.monotonic()
            _, error, _ = hb.wait_for_run_terminal(client, root, 90)
            if error:
                raise RuntimeError(error)
            result["seconds_after_release"] = time.monotonic() - released
            answer = json.loads((workspace / "answer.json").read_text())["answer"]
            if answer != sum(i * i for i in range(18)) % 997:
                result["errors"].append("incorrect worker result")
        result["agent"] = client.get_agent(root)
        if result["agent"]["status"]["state"] != ("cancelled" if cancel else "completed"):
            result["errors"].append("unexpected final agent status")
        if not cancel and "ANSWER=788" not in json.dumps(result["agent"]["status"]):
            result["errors"].append("agent did not report the expected answer")
    except Exception as error:
        result["errors"].append(f"{type(error).__name__}: {error}")
    finally:
        if client is not None:
            try:
                if run_id is not None:
                    result["events"] = hb.collect_run_events(client, run_id)
                if root is not None and client.get_agent(root)["status"]["state"] not in hb.TERMINAL_STATES:
                    error = hb._cancel_best_effort(client, root, 5)
                    if error:
                        result["errors"].append(f"cleanup: {error}")
            except Exception as error:
                result["errors"].append(f"capture/cleanup: {error}")
            finally:
                try:
                    client.close()
                except OSError as error:
                    result["errors"].append(f"client cleanup: {error}")
        error = hb.stop_daemon(binary, env, proc)
        if error:
            result["errors"].append(f"cleanup: {error}")
        result["status"] = "failed" if result["errors"] else "passed"
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ("status", "errors", "trial")}))
    return bool(result["errors"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/aether"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cancel", action="store_true")
    parser.add_argument("--monitor", action="store_true")
    args = parser.parse_args()
    raise SystemExit(run(str(args.binary.resolve()), args.output, args.cancel, args.monitor))
