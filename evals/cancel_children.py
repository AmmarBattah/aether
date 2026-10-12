"""Cancel working native-engine subtrees and verify durable outcomes and attribution."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time

from aether_client import AetherClient, init_git_workspace, isolated_env, protocol, start_daemon, stop_daemon


CASES = [
    ("luna-child", "claude-code/sonnet", "Luna", "codex/gpt-6-luna"),
    ("haiku-child", "codex/gpt-6-luna", "Haiku", "claude-code/haiku"),
]


def check(client, case, timeout, via_agent=False):
    name, parent_model, child_name, child_model = case
    root = client.create_agent(role="coordinator", model=parent_model)
    client.send(root["id"], f"Have one {child_name} agent review all the documentation and write review.md.")
    children = {}
    cancelled_at = None
    cancel_target = root["id"]
    cancellation_done = False
    waiting_since = None
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if via_agent and cancellation_done:
            state = client.get_agent(root["id"])["status"]["state"]
            if state == "waiting":
                if waiting_since is None:
                    waiting_since = time.monotonic()
                elif time.monotonic() - waiting_since >= 1:
                    break
            else:
                waiting_since = None
        try:
            frame = client.next_frame(timeout=min(0.2, max(0.1, deadline - time.monotonic())))
        except TimeoutError:
            continue
        if frame.get("frame") != "event":
            continue
        event = frame["event"]
        aid, kind = event.get("agent_id"), event["kind"]
        if kind["type"] == "agent.created" and aid != root["id"]:
            agent = client.get_agent(aid)
            if agent.get("parent") == root["id"]:
                children[aid] = agent
        if (aid in children and kind["type"] == "tool.started" and cancelled_at is None
                and client.get_agent(aid)["status"]["state"] == "running"):
            cancel_target = aid if via_agent else root["id"]
            if via_agent:
                client.send(root["id"], "Please stop that documentation review now. Cancel the reviewer and tell me when it has stopped.")
            else:
                client.request(protocol.cancel(agent=root["id"], reason="Operator stopped this review"))
            cancelled_at = event["seq"]
        if (aid == cancel_target and kind["type"] == "custom"
                and kind["name"] == "cancellation.completed"):
            cancellation_done = True
            if not via_agent:
                break

    events = []
    after = 0
    while True:
        batch = client.request(protocol.get_events(after=after, limit=5000, query={}))["data"]
        events.extend(batch)
        if len(batch) < 5000:
            break
        after = batch[-1]["seq"]
    agents = client.request(protocol.list_agents(query={}))["data"]
    errors = []
    if len(children) != 1 or len(agents) != 2:
        errors.append(f"Expected one supervisor and one child, found {len(agents)} agents/{len(children)} children")
    if cancelled_at is None:
        errors.append("No cancellation was sent during a child tool call")
    expected_provider, expected_model = child_model.split("/", 1)
    for child in children.values():
        if child["model"] != {"provider": expected_provider, "model": expected_model}:
            errors.append(f"Unexpected child model: {child['model']}")
    requested = [e for e in events if e.get("agent_id") == cancel_target
                 and e["kind"]["type"] == "custom" and e["kind"]["name"] == "cancellation.requested"]
    completed = [e for e in events if e.get("agent_id") == cancel_target
                 and e["kind"]["type"] == "custom" and e["kind"]["name"] == "cancellation.completed"]
    if len(requested) != 1 or len(completed) != 1:
        errors.append(f"Expected one durable request and completion, found {len(requested)}/{len(completed)}")
    if via_agent:
        calls = [e for e in events if e.get("agent_id") == root["id"]
                 and e["kind"]["type"] == "tool.requested" and e["kind"]["call"]["name"] in ("cancel_agent", "mcp__aether__cancel_agent")]
        if len(calls) != 1:
            errors.append(f"Expected one shared cancellation tool call, found {len(calls)}")
        if requested and requested[0]["actor"] != {"actor": "agent", "id": root["id"]}:
            errors.append("Cancellation was not attributed to the supervising agent")
    for agent in agents:
        if via_agent and agent["id"] == root["id"]:
            if agent["status"]["state"] != "waiting":
                errors.append(f"Supervisor did not settle after cancelling its child: {agent['status']}")
            continue
        stopped = [e for e in events if e.get("agent_id") == agent["id"]
                   and e["kind"]["type"] == "agent.cancelled"]
        if agent["status"]["state"] != "cancelled" or len(stopped) != 1:
            errors.append(f"Agent {agent['id']} did not cancel exactly once: {agent['status']}")
        elif requested and completed:
            if stopped[0]["actor"] != requested[0]["actor"]:
                errors.append(f"Agent {agent['id']} lost cancellation attribution")
            if not requested[0]["seq"] < stopped[0]["seq"] < completed[0]["seq"]:
                errors.append(f"Agent {agent['id']} has inconsistent cancellation ordering")
    return {"name": name, "via_agent": via_agent, "parent_model": parent_model, "child_model": child_model,
            "cancelled_after_tool_event": cancelled_at, "agents": agents, "events": events,
            "errors": errors, "passed": not errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--case", action="append", choices=[case[0] for case in CASES],
                        help="Repeat to select cases; defaults to all")
    parser.add_argument("--via-agent", action="store_true", help="Ask the supervisor to cancel its child using ordinary language")
    parser.add_argument("--sandbox-bin", help="Directory containing native sandbox dependencies, such as socat")
    args = parser.parse_args()
    selected = args.case or ["luna-child", "haiku-child"]
    binary = str(Path(args.binary).resolve())
    output = Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)
    results = {"binary_sha256": hashlib.sha256(Path(binary).read_bytes()).hexdigest(), "results": []}
    for case in CASES:
        if case[0] not in selected:
            continue
        with tempfile.TemporaryDirectory(prefix="aether-cancel-") as directory:
            env = isolated_env(directory)
            if args.sandbox_bin:
                env["PATH"] = str(Path(args.sandbox_bin).resolve()) + os.pathsep + env["PATH"]
            workspace = Path(directory) / "workspace"
            workspace.mkdir()
            (workspace / "README.md").write_text("# Documentation\nRead all the documents under docs.\n")
            docs = workspace / "docs"
            docs.mkdir()
            for index in range(20):
                (docs / f"service-{index}.md").write_text(
                    f"# Service {index}\nBackups run daily. Restore testing, retention, and ownership are unspecified.\n")
            init_git_workspace(workspace)
            try:
                start_daemon(binary, env, str(workspace))
                with AetherClient(env["AETHER_SOCKET"]) as client:
                    client.subscribe()
                    row = check(client, case, args.timeout, args.via_agent)
            except Exception as error:
                row = {"name": case[0], "passed": False, "errors": [str(error)]}
            finally:
                stop_daemon(binary, env)
            results["results"].append(row)
            output.write_text(json.dumps(results, indent=2) + "\n")
            print(json.dumps({key: row[key] for key in ("name", "passed", "errors")}), flush=True)
    raise SystemExit(0 if all(row["passed"] for row in results["results"]) else 1)


if __name__ == "__main__":
    main()
