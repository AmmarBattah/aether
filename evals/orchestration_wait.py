#!/usr/bin/env python3
"""Paid mixed-native orchestration smoke test, not an accuracy benchmark.

A child waits in a bounded local process for an external gate. The observer
holds that gate while the parent is parked, compares durable parent counters,
then releases it and verifies one resumption. Observer reads cost no model tokens.
All processes and data belong to a new isolated trial, never the user's daemon.
"""
import argparse
import json
import shutil
import tempfile
import time
from pathlib import Path

import harness_bench as hb
from aether_client import init_git_workspace
from harness_bench_usage import normalize_usage, reconcile_claude_receipts
from orchestration_cancel import cancel_parked_tree
from orchestration_timeout import WAIT_SECONDS, finish_timeout


CASES = {
    "sonnet-to-luna": ("claude-code/sonnet", "codex/gpt-6-luna"),
    "luna-to-sonnet": ("codex/gpt-6-luna", "claude-code/sonnet"),
}
WORKER = '''import json, os, pathlib, time
root = pathlib.Path(__file__).parent
(root / "worker.started").write_text(json.dumps({
    "pid": os.getpid(), "pid_namespace": os.stat("/proc/self/ns/pid").st_ino}))
deadline = time.monotonic() + 240
while not (root / "release.json").exists():
    if time.monotonic() >= deadline:
        raise TimeoutError("external test gate was not released")
    time.sleep(0.1)
release = json.loads((root / "release.json").read_text())
if release.get("fail"):
    (root / "failure.json").write_text(json.dumps({"error": "injected worker failure"}))
    raise RuntimeError("injected worker failure")
value = release["value"]
answer = sum(i * i for i in range(value + 1)) % 997
(root / "answer.json").write_text(json.dumps({"answer": answer}))
print(answer)
'''


def parent_counters(agent):
    stats = agent["stats"]
    return {key: stats[key] for key in ("turns", "usage", "tool_calls")}


def reconcile_claude_agent(tree, captures):
    agents = [node["agent"] for node in tree["nodes"].values()
              if node["agent"]["model"]["provider"] == "claude-code"]
    if len(agents) != 1:
        raise ValueError("receipt attribution requires exactly one Claude agent")
    agent = agents[0]
    durable = {"usage": normalize_usage(agent["stats"]["usage"], "aether"),
               "root_agent_id": agent["id"], "agent_ids": [agent["id"]]}
    return reconcile_claude_receipts(durable, [{**row, "agent_id": agent["id"]}
                                             for row in captures])


def wait_for_park(client, root, deadline):
    while time.monotonic() < deadline:
        frame = client.next_frame(timeout=max(0.01, deadline - time.monotonic()))
        if frame.get("frame") != "event":
            continue
        event = frame["event"]
        if event.get("agent_id") != root:
            continue
        kind = event["kind"]
        if kind["type"] == "agent.waiting":
            if kind["condition"]["on"] == "inbox":
                raise RuntimeError("parent went idle instead of waiting for its child")
            return event
        if kind["type"] in ("agent.failed", "agent.cancelled", "agent.completed"):
            raise RuntimeError(f"parent ended before parking: {kind['type']}")
    raise TimeoutError("parent never parked")


def observe_park(client, seconds):
    """Consume events without sending model requests or repeatedly querying state."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            client.next_frame(timeout=max(0.01, deadline - time.monotonic()))
        except TimeoutError:
            return


def assess(events, root, park_seq, release_seq, before, after):
    errors = []
    if before != after:
        errors.append("parent counters changed while the external gate was held")
    parent = [event for event in events if event.get("agent_id") == root]
    coordination = {event["kind"]["call"]["id"] for event in parent
                    if event["kind"]["type"] == "tool.requested"
                    and event["kind"]["call"]["name"].split("__")[-1] in ("delegate", "wait_for")}
    if any(event["kind"]["type"] == "tool.succeeded"
           and event["kind"]["call"] in coordination
           and event["kind"]["outcome"]["is_error"] for event in parent):
        errors.append("delegation or wait was reported as a tool error")
    forbidden = {"agent.started", "snapshot.created", "agent.usage_updated", "tool.requested"}
    unexpected = [event["kind"]["type"] for event in parent
                  if park_seq < event["seq"] <= release_seq
                  and event["kind"]["type"] in forbidden]
    if unexpected:
        errors.append(f"parent work occurred during its unresolved wait: {unexpected}")
    resumes = [event for event in parent if event["seq"] > release_seq
               and event["kind"]["type"] == "agent.started"]
    if len(resumes) != 1:
        errors.append(f"expected one parent resumption, observed {len(resumes)}")
    return errors


def run_case(binary, name, output, timeout, hold, explicit_completion=False, cancel=False,
             child_failure=False, wait_timeout=False):
    parent_model, child_model = CASES[name]
    trial = Path(tempfile.mkdtemp(prefix="ow-", dir=hb.SCRATCH_ROOT))
    workspace = trial / "workspace"
    workspace.mkdir()
    (workspace / "worker.py").write_text(WORKER)
    init_git_workspace(str(workspace))
    env = hb.isolated_env(str(trial))
    stripped = hb.strip_live_routing_env(env, keep=hb._AETHER_ISOLATED_ENV_KEYS)
    receipts = hb.install_claude_receipt_proxy(env, str(trial), shutil.which("claude"))
    child_completion = (
        "After the worker exits, explicitly call complete_task with the answer in its summary "
        "and structured result, so the test exercises terminal tool reporting. "
        if explicit_completion else "Return the computed answer. "
    )
    worker_outcome = "then writes answer.json. "
    delegation_wait = "wait=true and leaf=true"
    parent_outcome = "Once the child finishes, read answer.json and report ANSWER=<value> in your final answer. "
    if child_failure:
        worker_outcome = "then deliberately raises an injected failure and writes failure.json. "
        child_completion = (
            "When the worker fails, explicitly call fail_task with error 'injected worker failure'. "
            "Do not retry, repair the fixture, or manufacture an answer. "
        )
        parent_outcome = (
            "Once the child fails, report CHILD_FAILED in your final answer and finish your own task. "
            "Do not retry, spawn another child, or claim the worker succeeded. "
        )
    if wait_timeout:
        delegation_wait = "wait=false and leaf=true"
        parent_outcome = (
            f"Immediately after delegation, call wait_for with the returned task_id and timeout_seconds={WAIT_SECONDS}. "
            "The external gate will stay closed. After the timeout wakes you, call cancel_agent on your child "
            "once, report TIMED_OUT in your final answer, and finish. Do not release the gate or retry. "
        )
    prompt = (
        "This is an orchestration integration test. Delegate exactly one child using "
        f"model {child_model} and effort low, with {delegation_wait}. "
        "Give the child this objective: Run python3 worker.py once in the shared workspace; "
        "it waits up to 240 seconds for an external fixture gate, " + worker_outcome +
        "Do not create or modify release.json or worker.py. "
        + child_completion +
        "As parent, let Aether wait for the child; do not poll, sleep, or run the worker yourself. "
        + parent_outcome + "Work only inside this workspace and do not access the network."
    )
    result = {"name": name, "parent_model": parent_model, "child_model": child_model,
              "explicit_child_completion": explicit_completion, "cancel_while_parked": cancel,
              "child_failure": child_failure,
              "wait_timeout": wait_timeout,
              "effort": "low", "trial_dir": str(trial), "prompt": prompt,
              "stripped_env_keys": stripped, "errors": [], "status": "running"}
    proc = client = root = run = None
    started = time.monotonic()
    try:
        proc = hb.start_daemon(binary, env, str(workspace))
        client = hb.AetherClient(env["AETHER_SOCKET"])
        client.subscribe()
        agent = client.create_agent(objective=prompt, model=parent_model,
                                    effort="low", long_lived=False)
        root, run = agent["id"], agent["run"]
        result["root_agent_id"] = root
        deadline = time.monotonic() + timeout
        parked = wait_for_park(client, root, deadline)
        before = parent_counters(client.get_agent(root))
        hold_started = time.monotonic()
        if deadline - time.monotonic() < hold:
            raise TimeoutError("not enough time remains for the full wait observation")
        observe_park(client, hold)
        after_agent = client.get_agent(root)
        after = parent_counters(after_agent)
        events = hb.collect_run_events(client, run)
        release_seq = max(event["seq"] for event in events)
        result["wait_observation"] = {"park_seq": parked["seq"], "release_seq": release_seq,
                                      "seconds": time.monotonic() - hold_started,
                                      "before": before, "after": after}
        if after_agent["status"]["state"] != "waiting":
            result["errors"].append("parent did not remain waiting")
        if (workspace / "release.json").exists():
            raise RuntimeError("an agent released the fixture gate")
        if cancel:
            cancellation = cancel_parked_tree(
                client, root, run, parked, before, child_model, workspace, observe_park)
            result["cancellation"] = cancellation
            result["errors"].extend(cancellation["errors"])
            if (workspace / "worker.py").read_text() != WORKER:
                result["errors"].append("fixture worker was modified")
            return result
        if wait_timeout:
            timed, events = finish_timeout(client, root, run, parked, workspace, child_model, deadline)
            result["timeout_observation"] = timed
            result["errors"].extend(timed["errors"])
            result["errors"].extend(assess(events, root, parked["seq"], release_seq, before, after))
            if (workspace / "worker.py").read_text() != WORKER:
                result["errors"].append("fixture worker was modified")
            return result
        value = 137
        expected = sum(i * i for i in range(value + 1)) % 997
        (workspace / "release.json").write_text(json.dumps({"fail": True} if child_failure else {"value": value}))
        status, error, _ = hb.wait_for_run_terminal(client, root, max(0.01, deadline - time.monotonic()))
        result["workflow_status"] = status
        if status != "completed":
            result["errors"].append(error or f"parent ended {status}")
        tree, quiescent = hb.wait_for_tree_quiescent(client, run, root, 15)
        if not quiescent:
            result["errors"].append("tree did not become quiescent")
        children = [node["agent"] for aid, node in tree["nodes"].items() if aid != root]
        provider, model = child_model.split("/", 1)
        if (len(children) != 1 or children[0]["parent"] != root
                or children[0]["model"] != {"provider": provider, "model": model}
                or children[0]["config"].get("effort") != "low"
                or children[0]["status"]["state"] != ("failed" if child_failure else "completed")):
            result["errors"].append("child count, model, effort, parent, or terminal state mismatched")
        events = hb.collect_run_events(client, run)
        result["errors"].extend(assess(events, root, parked["seq"], release_seq, before, after))
        terminal_tool = "fail_task" if child_failure else "complete_task"
        if (explicit_completion or child_failure) and children and not any(
            event.get("agent_id") == children[0]["id"]
            and event["kind"]["type"] == "tool.requested"
            and event["kind"]["call"]["name"].split("__")[-1] == terminal_tool
            for event in events
        ):
            result["errors"].append(f"child did not exercise explicit {terminal_tool}")
        resumes = [event["seq"] for event in events if event.get("agent_id") == root
                   and event["kind"]["type"] == "agent.started" and event["seq"] > release_seq]
        completions = [event["seq"] for event in events if children
                       and event.get("agent_id") == children[0]["id"]
                       and event["kind"]["type"] == ("agent.failed" if child_failure else "agent.completed")]
        if not completions or not resumes or min(resumes) <= max(completions):
            result["errors"].append("parent did not resume strictly after child terminal state")
        if child_failure:
            if json.loads((workspace / "failure.json").read_text()) != {"error": "injected worker failure"}:
                result["errors"].append("injected failure artifact was incorrect")
            if (workspace / "answer.json").exists():
                result["errors"].append("an answer was manufactured despite injected worker failure")
        elif json.loads((workspace / "answer.json").read_text()) != {"answer": expected}:
            result["errors"].append("child output was incorrect")
        replies = [block.get("text", "") for event in events if event.get("agent_id") == root
                   and event["kind"]["type"] == "agent.message"
                   and event["kind"].get("role") == "assistant"
                   for block in event["kind"]["content"] if block.get("type") == "text"]
        expected_reply = "CHILD_FAILED" if child_failure else f"ANSWER={expected}"
        if not replies or expected_reply not in replies[-1]:
            result["errors"].append("parent did not return the expected final answer")
        if (workspace / "worker.py").read_text() != WORKER:
            result["errors"].append("fixture worker was modified")
    except Exception as error:
        result["errors"].append(f"{type(error).__name__}: {error}")
    finally:
        if client is not None:
            try:
                if run is not None:
                    tree = hb.fetch_tree(client, run)
                    cleanup_deadline = time.monotonic() + 10
                    for aid, node in tree["nodes"].items():
                        if node["agent"]["status"]["state"] not in hb.TERMINAL_STATES:
                            remaining = cleanup_deadline - time.monotonic()
                            if remaining <= 0:
                                result["errors"].append("cleanup deadline exhausted; stopping daemon")
                                break
                            error = hb._cancel_best_effort(client, aid, min(5, remaining))
                            if error:
                                result["errors"].append(f"cancel: {error}")
            except Exception as error:
                result["errors"].append(f"cleanup: {type(error).__name__}: {error}")
            finally:
                try:
                    client.close()
                except OSError as error:
                    result["errors"].append(f"client close: {error}")
        error = hb.stop_daemon(binary, env, proc)
        if error:
            result["errors"].append(f"daemon shutdown: {error}")
        if run is not None:
            try:
                hb.capture_stopped_run(env, run, root, str(trial), result, "low")
            except Exception as error:
                result["errors"].append(f"capture: {type(error).__name__}: {error}")
        result["claude_receipts"] = hb.read_claude_receipts(receipts)
        result["claude_receipt_coverage"] = {
            "processes": len(result["claude_receipts"]),
            "terminal_receipts": sum(row.get("receipt") is not None for row in result["claude_receipts"]),
            "note": "Process receipts are separate from durable totals; never add the two.",
        }
        result["functional_status"] = "failed" if result["errors"] else "passed"
        if result.get("usage_error"):
            result["errors"].append(f"usage accounting: {result['usage_error']}")
        coverage = result["claude_receipt_coverage"]
        if not coverage["processes"] or coverage["terminal_receipts"] != coverage["processes"]:
            result["errors"].append("Claude terminal receipt coverage is incomplete")
        try:
            tree = json.loads(Path(result["raw_logs"]["tree"]).read_text())
            reconciliation = reconcile_claude_agent(tree, result["claude_receipts"])
            result["claude_reconciliation"] = reconciliation
            cost = "api_equivalent_cost_usd"
            difference = reconciliation["native_minus_durable"][cost]
            if (not reconciliation["coverage"][cost]["complete"] or difference is None
                    or abs(difference) > 1e-9):
                result["errors"].append("Claude session cost does not fully reconcile with durable usage")
        except (KeyError, OSError, ValueError) as error:
            result["errors"].append(f"Claude reconciliation: {type(error).__name__}: {error}")
        result["elapsed_seconds"] = time.monotonic() - started
        result["status"] = "failed" if result["errors"] else "passed"
        hb.save_json(str(output / f"{name}.json"), result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--cases", default=",".join(CASES))
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--hold", type=float, default=20)
    parser.add_argument("--explicit-completion", action="store_true",
                        help="require the child to use complete_task, exercising terminal reporting")
    parser.add_argument("--cancel", action="store_true",
                        help="cancel the parked parent and verify subtree cancellation without resumption")
    parser.add_argument("--child-failure", action="store_true",
                        help="release the worker into an injected failure and verify one parent resumption")
    parser.add_argument("--wait-timeout", action="store_true",
                        help="keep the gate closed and verify deadline wakeup followed by child cancellation")
    args = parser.parse_args()
    names = args.cases.split(",")
    if any(name not in CASES for name in names) or not 0 < args.hold < args.timeout:
        parser.error("unknown case or invalid positive hold/timeout")
    if sum((args.cancel, args.explicit_completion, args.child_failure, args.wait_timeout)) > 1:
        parser.error("cancellation, child failure, and explicit completion are separate scenarios")
    if args.wait_timeout and not args.hold < WAIT_SECONDS < args.timeout:
        parser.error("timeout scenario requires hold < 40 seconds < overall timeout")
    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=True)
    Path(hb.SCRATCH_ROOT).mkdir(parents=True, exist_ok=True)
    binary, digest = hb.pin_binary(str(Path(args.binary).resolve()), str(output))
    hb.save_json(str(output / "provenance.json"), {
        "binary_sha256": digest, "harness_sha256": hb.sha256_file(__file__),
        "cancellation_helper_sha256": hb.sha256_file(str(Path(__file__).with_name("orchestration_cancel.py"))),
        "worker_observer_sha256": hb.sha256_file(str(Path(__file__).with_name("orchestration_worker.py"))),
        "timeout_observer_sha256": hb.sha256_file(str(Path(__file__).with_name("orchestration_timeout.py"))),
        "benchmark_helper_hashes": hb.benchmark_hashes(),
        "claude": hb.native_cli_provenance("claude"), "codex": hb.native_cli_provenance("codex"),
        "scenario": "cancellation" if args.cancel else "child_failure" if args.child_failure else "timeout" if args.wait_timeout else "completion",
        "limitation": "Lifecycle smoke only; timeout/restart, full OS process-tree cleanup, and hard accuracy need separate trials.",
    })
    results = []
    for name in names:
        result = run_case(binary, name, output, args.timeout, args.hold, args.explicit_completion,
                          args.cancel, args.child_failure, args.wait_timeout)
        results.append(result)
        print(json.dumps({"name": name, "status": result["status"], "errors": result["errors"]}), flush=True)
    raise SystemExit(0 if all(row["status"] == "passed" for row in results) else 1)


if __name__ == "__main__":
    main()
