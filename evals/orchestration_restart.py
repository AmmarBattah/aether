#!/usr/bin/env python3
"""Paid native deadline recovery drill; kills only its owned isolated daemon.

No tool is in flight at the crash boundary. This tests durable parked waits,
not automatic replay of interrupted mutations or cleanup of orphaned children.
"""
import argparse
from datetime import datetime
import json
from pathlib import Path
import shutil
import tempfile
import time

import harness_bench as hb
from aether_client import init_git_workspace
from orchestration_timeout import deadlines
from orchestration_wait import observe_park, parent_counters, reconcile_claude_agent, wait_for_park


MODELS = {"sonnet": "claude-code/sonnet", "luna": "codex/gpt-6-luna"}


def assess(events, root, parked, before, recovered, after):
    errors = []
    if before != recovered or before != after:
        errors.append("parked counters changed across restart or idle observation")
    dates = deadlines(parked["kind"]["condition"])
    resumes = [e for e in events if e.get("agent_id") == root
               and e["seq"] > parked["seq"] and e["kind"]["type"] == "agent.started"]
    if len(dates) != 1 or len(resumes) != 1:
        errors.append("expected one persisted deadline and one resumed turn")
    elif datetime.fromisoformat(resumes[0]["at"]) < datetime.fromisoformat(dates[0]):
        errors.append("agent resumed before its original deadline")
    calls = [e["kind"]["call"] for e in events if e.get("agent_id") == root
             and e["kind"]["type"] == "tool.requested"]
    waits = [c for c in calls if c["name"].split("__")[-1] == "wait_for"]
    if len(waits) != 1 or waits[0]["input"] != {"timeout_seconds": 60}:
        errors.append("agent did not use exactly one 60-second deadline wait")
    if any(c["name"].split("__")[-1] not in ("wait_for", "complete_task") for c in calls):
        errors.append("agent performed unexpected tool work")
    replies = [b.get("text", "") for e in events if e.get("agent_id") == root
               and e["kind"]["type"] == "agent.message" and e["kind"].get("role") == "assistant"
               for b in e["kind"]["content"] if b.get("type") == "text"]
    if not replies or "RESTART_OK" not in replies[-1]:
        errors.append("final response omitted RESTART_OK")
    return errors


def connect_restarted(proc, socket):
    # A killed daemon can leave its socket path until its successor binds.
    deadline = time.monotonic() + 15
    last_error = None
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"restarted daemon exited {proc.returncode}")
        try:
            return hb.AetherClient(socket)
        except (FileNotFoundError, ConnectionRefusedError) as error:
            last_error = error
            time.sleep(0.05)
    raise TimeoutError("restarted daemon did not accept connections") from last_error


def run_case(binary, name, output):
    trial = Path(tempfile.mkdtemp(prefix="restart-", dir=hb.SCRATCH_ROOT))
    workspace = trial / "workspace"
    workspace.mkdir()
    init_git_workspace(str(workspace))
    env = hb.isolated_env(str(trial))
    stripped = hb.strip_live_routing_env(env, keep=hb._AETHER_ISOLATED_ENV_KEYS)
    receipts = hb.install_claude_receipt_proxy(env, str(trial), shutil.which("claude"))
    result = {"name": name, "model": MODELS[name], "effort": "low", "trial_dir": str(trial),
              "stripped_env_keys": stripped, "errors": []}
    proc = client = root = run = None
    started = time.monotonic()
    try:
        proc = hb.start_daemon(binary, env, str(workspace))
        client = hb.AetherClient(env["AETHER_SOCKET"])
        client.subscribe()
        agent = client.create_agent(model=MODELS[name], effort="low", long_lived=False,
            objective="This is a daemon restart integration test. Call wait_for exactly once with "
            "only timeout_seconds=60. Let Aether wait; do not poll, sleep, delegate, or use shell/file "
            "tools. After the deadline wakes you, report RESTART_OK in your final response and finish.")
        root, run = agent["id"], agent["run"]
        result["root_agent_id"] = root
        parked = wait_for_park(client, root, time.monotonic() + 120)
        before = parent_counters(client.get_agent(root))
        observe_park(client, 5)
        if parent_counters(client.get_agent(root)) != before:
            raise RuntimeError("agent did not settle before crash")
        # This Popen owns a dedicated foreground daemon; never signal a PID read
        # from a socket file or a user-owned daemon, and never kill a broad group.
        proc.kill()
        result["crash_exit_code"] = proc.wait(timeout=10)
        shutil.copyfile(trial / "daemon-stderr.log", trial / "daemon-before-crash.log")
        client.close()
        client = None
        proc = hb.start_daemon(binary, env, str(workspace))
        client = connect_restarted(proc, env["AETHER_SOCKET"])
        client.subscribe(after=parked["seq"])
        recovered = client.get_agent(root)
        if recovered["status"] != {"state": "waiting", "condition": parked["kind"]["condition"]}:
            raise RuntimeError("restart changed the persisted wait condition")
        recovered_counters = parent_counters(recovered)
        observe_park(client, 10)
        after = parent_counters(client.get_agent(root))
        result["idle_counters"] = {"before": before, "recovered": recovered_counters, "after": after}
        status, error, _ = hb.wait_for_run_terminal(client, root, 120)
        if status != "completed":
            result["errors"].append(error or f"agent ended {status}")
        events = hb.collect_run_events(client, run)
        result["errors"].extend(assess(events, root, parked, before, recovered_counters, after))
    except Exception as error:
        result["errors"].append(f"{type(error).__name__}: {error}")
    finally:
        if client is not None:
            try:
                if root and client.get_agent(root)["status"]["state"] not in hb.TERMINAL_STATES:
                    error = hb._cancel_best_effort(client, root, 5)
                    if error:
                        result["errors"].append(f"cleanup cancellation: {error}")
            except Exception as error:
                result["errors"].append(f"cleanup: {error}")
            finally:
                try:
                    client.close()
                except OSError as error:
                    result["errors"].append(f"client close: {error}")
        error = hb.stop_daemon(binary, env, proc)
        if error:
            result["errors"].append(f"shutdown: {error}")
        if run:
            try:
                hb.capture_stopped_run(env, run, root, str(trial), result, "low")
                if result.get("usage_error"):
                    result["errors"].append(result["usage_error"])
                if name == "sonnet":
                    captures = hb.read_claude_receipts(receipts)
                    result["claude_receipts"] = captures
                    if len(captures) != 2 or any(c.get("receipt") is None for c in captures):
                        result["errors"].append("expected two Claude terminal receipts")
                    tree = json.loads(Path(result["raw_logs"]["tree"]).read_text())
                    reconciliation = reconcile_claude_agent(tree, captures)
                    result["claude_reconciliation"] = reconciliation
                    differences = reconciliation["native_minus_durable"]
                    if any(value is None or abs(value) > 1e-9 for value in differences.values()):
                        result["errors"].append("Claude usage did not reconcile")
            except Exception as error:
                result["errors"].append(f"capture: {error}")
        result["elapsed_seconds"] = time.monotonic() - started
        result["status"] = "failed" if result["errors"] else "passed"
        hb.save_json(str(output / f"{name}.json"), result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=True)
    Path(hb.SCRATCH_ROOT).mkdir(parents=True, exist_ok=True)
    binary, digest = hb.pin_binary(str(Path(args.binary).resolve()), str(output))
    hb.save_json(str(output / "provenance.json"), {
        "binary_sha256": digest, "harness_sha256": hb.sha256_file(__file__),
        "benchmark_helper_hashes": hb.benchmark_hashes(),
        "claude": hb.native_cli_provenance("claude"), "codex": hb.native_cli_provenance("codex"),
        "scope": "SIGKILL of isolated daemon after agent parked; no in-flight mutation or child",
    })
    results = [run_case(binary, name, output) for name in MODELS]
    for result in results:
        print(json.dumps({k: result[k] for k in ("name", "status", "errors")}), flush=True)
    raise SystemExit(0 if all(r["status"] == "passed" for r in results) else 1)


if __name__ == "__main__":
    main()
