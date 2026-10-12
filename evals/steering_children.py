"""Live corrections to working delegated children, checked against durable events and files."""
import argparse
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time

from aether_client import AetherClient, init_git_workspace, isolated_env, start_daemon, stop_daemon

CASES = [
    ("luna-child", "claude-code/sonnet", "Luna", "codex/gpt-6-luna"),
    ("codex-child", "claude-code/sonnet", "Sol", "codex/gpt-6.1-sol"),
    ("claude-child", "codex/gpt-6-luna", "Sonnet", "claude-code/sonnet"),
    ("haiku-child", "codex/gpt-6-luna", "Haiku", "claude-code/haiku"),
]


def check(client, workspace, case, timeout):
    name, parent_model, child_name, child_model = case
    correction = (
        "Actually, focus your review on backup and recovery. "
        "Put the review in recovery-review.md instead. You can skip the broad review."
    )
    root = client.create_agent(role="coordinator", model=parent_model)
    client.send(root["id"], f"Have one {child_name} agent review the documentation and write review.md. Let me know when it is done.")
    children = {}
    child_id = None
    sent = None
    turn_at_send = None
    report_at_turn_end = None
    report_mtime_at_turn_end = None
    events = []
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            frame = client.next_frame(timeout=min(10, max(0.1, deadline - time.monotonic())))
        except TimeoutError:
            continue
        if frame.get("frame") != "event":
            continue
        event = frame["event"]
        kind = event["kind"]
        aid = event.get("agent_id")
        if kind["type"] == "agent.created" and aid != root["id"]:
            agent = client.get_agent(aid)
            if agent.get("parent") == root["id"]:
                children[aid] = agent
        if aid in children:
            events.append(event)
            if sent is None and kind["type"] == "tool.started":
                current = client.get_agent(aid)
                if current["status"]["state"] == "running":
                    child_id = aid
                    turn_at_send = next(e["kind"]["turn"] for e in reversed(events)
                                        if e["kind"]["type"] == "agent.started")
                    sent = client.send(aid, correction)
            if (aid == child_id and kind["type"] == "agent.turn_completed"
                    and kind["turn"] == turn_at_send):
                path = workspace / "recovery-review.md"
                if path.exists():
                    report_at_turn_end = path.read_text()
                    report_mtime_at_turn_end = path.stat().st_mtime
            if kind["type"] in ("agent.completed", "agent.failed", "agent.cancelled"):
                children[aid] = client.get_agent(aid)
                break
        if aid == root["id"] and kind["type"] in ("agent.failed", "agent.cancelled"):
            break
    errors = []
    if len(children) != 1:
        errors.append(f"Expected one delegated child, found {len(children)}")
    if sent is None:
        errors.append("No correction was sent while a child was running a tool")
    target = children.get(child_id)
    if target is not None:
        expected_provider, expected_model = child_model.split("/", 1)
        if target["model"] != {"provider": expected_provider, "model": expected_model}:
            errors.append(f"Unexpected child model: {target['model']}")
        if target["status"]["state"] != "completed":
            errors.append(f"Child did not complete: {target['status']}")
    message_id = sent.get("message") if sent else None
    delivered = [e for e in events if e["kind"]["type"] == "inbox.delivered"
                 and e["kind"]["item"]["kind"].get("message_id") == message_id]
    inbox_ids = {e["kind"]["item"]["id"] for e in delivered}
    consumed = [e for e in events if e["kind"]["type"] == "inbox.consumed"
                and inbox_ids.intersection(e["kind"]["items"])]
    if len(delivered) != 1 or len(consumed) != 1:
        errors.append(f"Expected one delivery and consumption, found {len(delivered)}/{len(consumed)}")
    completions = [e for e in events if e["kind"]["type"] == "agent.turn_completed"]
    if not completions or completions[0]["kind"]["turn"] != turn_at_send:
        errors.append("No completion event for the steered runtime turn")
    if (completions and report_mtime_at_turn_end is not None
            and report_mtime_at_turn_end > datetime.fromisoformat(completions[0]["at"]).timestamp()):
        errors.append("Report was modified after the steered turn completed")
    if report_at_turn_end is None or not report_at_turn_end.strip():
        errors.append("The revised report was not written during the steered turn")
    elif not all(term in report_at_turn_end.lower() for term in ("restor", "retention")):
        errors.append("The steered turn's report omits restore-testing or retention findings")
    if consumed and completions and consumed[0]["seq"] >= completions[0]["seq"]:
        errors.append("Correction was consumed after the turn completed")
    report_file = workspace / "recovery-review.md"
    report = report_file.read_text() if report_file.exists() else None
    if report is None or not report.strip():
        errors.append("Written report does not follow the correction")
    if report is not None and not all(term in report.lower() for term in ("restor", "retention")):
        errors.append("Report omits the fixture restore-testing or retention gap")
    return {"name": name, "parent_model": parent_model, "child_model": child_model,
            "correction": correction, "message": sent, "turn_at_send": turn_at_send,
            "children": list(children.values()), "events": events,
            "report": report, "report_at_turn_end": report_at_turn_end,
            "report_mtime_at_turn_end": report_mtime_at_turn_end,
            "extra_completion_turns": max(0, len(completions) - 1),
            "errors": errors, "passed": not errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--sandbox-bin", help="Directory containing native sandbox dependencies, such as socat")
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--case", action="append", choices=[case[0] for case in CASES],
                        help="Repeat to select cases; defaults to Sol, Sonnet and Haiku; Luna is opt-in")
    args = parser.parse_args()
    selected = args.case or ["codex-child", "claude-child", "haiku-child"]
    binary = str(Path(args.binary).resolve())
    output = Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)
    results = {"binary_sha256": hashlib.sha256(Path(binary).read_bytes()).hexdigest(), "results": []}
    for case in CASES:
        if case[0] not in selected:
            continue
        with tempfile.TemporaryDirectory(prefix="aether-steering-") as directory:
            env = isolated_env(directory)
            if args.sandbox_bin:
                env["PATH"] = str(Path(args.sandbox_bin).resolve()) + os.pathsep + env["PATH"]
            workspace = Path(directory) / "workspace"
            workspace.mkdir()
            (workspace / "README.md").write_text("# Documentation review fixture\n\nRead the documents under docs.\n")
            docs = workspace / "docs"
            docs.mkdir()
            (docs / "recovery.md").write_text(
                "# Backup and recovery\n\nBackups run daily, but restores have never been tested. "
                "Retention is unspecified. The recovery owner and recovery time objective are undefined.\n")
            (docs / "deployment.md").write_text(
                "# Deployment\n\nAn operator deploys a tagged release after tests pass. "
                "Rollback steps are missing. Database migrations must preserve existing records.\n")
            init_git_workspace(workspace)
            try:
                start_daemon(binary, env, str(workspace))
                with AetherClient(env["AETHER_SOCKET"]) as client:
                    client.subscribe()
                    row = check(client, workspace, case, args.timeout)
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
