"""Live acceptance of ordinary user requests, verified against Aether's stored tree."""
import argparse
from datetime import datetime, timezone
import json
import hashlib
from pathlib import Path
import tempfile
import time

from aether_client import AetherClient, init_git_workspace, isolated_env, start_daemon, stop_daemon, protocol


def tool_failed(kind):
    return kind["type"] == "tool.failed" or (
        kind["type"] == "tool.succeeded" and kind["outcome"].get("is_error", False)
    )


def tree_errors(agents, parent, expected, *, in_progress=False):
    children = [agent for agent in agents if agent.get("parent") == parent]
    minimum = expected.get("count", expected.get("min_count", 0))
    maximum = expected.get("count", expected.get("max_count", minimum))
    errors = []
    if len(children) > maximum or (not in_progress and len(children) < minimum):
        errors.append(f"{parent}: expected {minimum}..{maximum} children, got {len(children)}")
    for child in children:
        identity = child["id"]
        model = {key: expected[key] for key in ("provider", "model")}
        if child["model"] != model:
            errors.append(f"{identity}: expected {model}, got {child['model']}")
        if "effort" in expected and child["config"].get("effort") != expected["effort"]:
            errors.append(f"{identity}: expected effort {expected['effort']}")
        if not in_progress and child["status"]["state"] != "completed":
            errors.append(f"{identity}: did not complete: {child['status']}")
        if not in_progress and not child["status"].get("summary", "").strip():
            errors.append(f"{identity}: no work result recorded")
        if "children" in expected:
            errors.extend(tree_errors(agents, identity, expected["children"], in_progress=in_progress))
        elif any(agent.get("parent") == identity for agent in agents):
            errors.append(f"{identity}: spawned unexpected descendants")
    return errors


def first_turn_errors(events, parent, expected_count):
    """Count durable direct children between the root's first start and turn end."""
    started = False
    children = set()
    for event in events:
        kind = event['kind']
        if event.get('agent_id') == parent:
            if kind['type'] == 'agent.started' and not started:
                if kind.get('turn') != 1:
                    return ['The observed root turn was not turn 1']
                started = True
            elif started and kind['type'] in ('agent.turn_completed', 'agent.interrupted'):
                if len(children) == expected_count:
                    return []
                return [f'First turn created {len(children)} children; expected {expected_count}']
        if started and kind['type'] == 'agent.created' and kind.get('parent') == parent:
            children.add(event['agent_id'])
    return ['No complete first-turn boundary was observed']


def case_display_name(case):
    """The name to show in a report, even for a case whose own name is malformed."""
    name = case.get("name")
    return name if isinstance(name, str) and name else "<unnamed>"


def validate_expected_tree(expected, path):
    """Schema errors in one `children`/`tree`/`grandchildren_per_child` level."""
    if not isinstance(expected, dict):
        return [f"{path}: must be an object, got {type(expected).__name__}"]
    errors = []
    has_count = "count" in expected
    has_range = "min_count" in expected or "max_count" in expected
    if has_count == has_range:
        errors.append(f"{path}: need exactly one of 'count' or 'min_count'/'max_count'")
    for key in ("count", "min_count", "max_count"):
        if key in expected and not isinstance(expected[key], int):
            errors.append(f"{path}: '{key}' must be an integer")
    if "provider" not in expected or not isinstance(expected.get("provider"), str):
        errors.append(f"{path}: 'provider' must be a string")
    if "model" not in expected or not isinstance(expected.get("model"), str):
        errors.append(f"{path}: 'model' must be a string")
    if "effort" in expected and not isinstance(expected["effort"], str):
        errors.append(f"{path}: 'effort' must be a string")
    if "children" in expected:
        errors.extend(validate_expected_tree(expected["children"], f"{path}.children"))
    return errors


def validate_case_schema(case):
    """Schema-only validation: shape, no daemon, no model call (P2's --dry-run)."""
    errors = []
    if not isinstance(case.get("name"), str) or not case["name"]:
        errors.append("'name' must be a non-empty string")
    name = case_display_name(case)
    if not isinstance(case.get("prompt"), str) or not case["prompt"]:
        errors.append(f"{name}: 'prompt' must be a non-empty string")
    model = case.get("model")
    if not isinstance(model, str) or "/" not in model:
        errors.append(f"{name}: 'model' must be 'provider/model'")
    if "effort" in case and not isinstance(case["effort"], str):
        errors.append(f"{name}: 'effort' must be a string")
    expected = case.get("tree", case.get("children"))
    if expected is None:
        errors.append(f"{name}: needs a 'tree' or 'children' expectation")
    else:
        errors.extend(validate_expected_tree(expected, f"{name}.{'tree' if 'tree' in case else 'children'}"))
    if "grandchildren_per_child" in case:
        errors.extend(validate_expected_tree(case["grandchildren_per_child"], f"{name}.grandchildren_per_child"))
    if "first_turn_children" in case and not isinstance(case["first_turn_children"], int):
        errors.append(f"{name}: 'first_turn_children' must be an integer")
    if "events_required" in case:
        required = case["events_required"]
        if not isinstance(required, list) or not all(isinstance(item, str) for item in required):
            errors.append(f"{name}: 'events_required' must be a list of strings")
    return errors


def checkpoint_carries_forward_errors(events, agent_id):
    """Once a checkpoint exists, the very next snapshot for this agent must
    render a non-empty checkpoint layer - proof the summary reached the next
    turn's prompt, not just that a checkpoint record was written."""
    after_checkpoint = False
    for event in events:
        if event.get("agent_id") != agent_id:
            continue
        kind = event["kind"]
        if kind["type"] == "checkpoint.created":
            after_checkpoint = True
            continue
        if after_checkpoint and kind["type"] == "snapshot.created":
            layer = next((l for l in kind["layers"] if l["kind"] == "checkpoint"), None)
            if layer is None or not (layer.get("text") or "").strip():
                return ["next turn's snapshot did not carry the checkpoint summary forward"]
            return []
    if after_checkpoint:
        return ["no snapshot followed the checkpoint; summary carry-forward not observed"]
    return []


def event_requirement_errors(case, events, agent_id):
    """Optional extra event-shape assertions declared by `events_required`."""
    required = case.get("events_required")
    if not required:
        return []
    seen = {event["kind"]["type"] for event in events if event.get("agent_id") == agent_id}
    errors = [f"missing required event: {event_type}" for event_type in required if event_type not in seen]
    if "checkpoint.created" in required:
        errors.extend(checkpoint_carries_forward_errors(events, agent_id))
    return errors


def check(client, case, timeout):
    root = client.create_agent(role="coordinator", model=case["model"], effort=case.get("effort"))
    start = len(client.events_seen)
    client.send(root["id"], case["prompt"])
    deadline = time.monotonic() + timeout
    fleet = [root]
    timed_out = False
    expected = case.get("tree", case.get("children"))
    if "grandchildren_per_child" in case:
        expected = {**expected, "children": case["grandchildren_per_child"]}
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            timed_out = True
            break
        try:
            frame = client.next_frame(timeout=min(remaining, 10))
        except TimeoutError:
            continue
        if frame.get("frame") != "event":
            continue
        kind = frame["event"]["kind"]["type"]
        if kind not in ("agent.created", "agent.waiting", "agent.turn_completed", "agent.completed", "agent.failed", "agent.cancelled"):
            continue
        fleet = client.request(protocol.list_agents(query={"run": root["run"]}))["data"]
        if isinstance(fleet, dict):
            fleet = fleet["agents"]
        if tree_errors(fleet, root["id"], expected, in_progress=True):
            break
        current_root = next(agent for agent in fleet if agent["id"] == root["id"])
        root_state = current_root["status"]["state"]
        descendants = [agent for agent in fleet if agent["id"] != root["id"]]
        if root_state in ("failed", "cancelled"):
            break
        first_turn_observed = case.get("first_turn_children") is None or any(
            event.get("agent_id") == root["id"]
            and event["kind"]["type"] == "agent.turn_completed"
            and event["kind"].get("turn") == 1
            for event in client.events_seen[start:]
        )
        if first_turn_observed and root_state == "waiting" and current_root["inbox_cursor"] > 0 and all(
            agent["status"]["state"] in ("completed", "failed", "cancelled") for agent in descendants
        ):
            break
    errors = tree_errors(fleet, root["id"], expected)
    root_status = next(agent["status"] for agent in fleet if agent["id"] == root["id"])
    if root_status["state"] != "waiting":
        errors.append(f"Coordinator did not finish successfully: {root_status}")
    if timed_out:
        errors.append(f"Timed out after {timeout}s")
    identifiers = {agent["id"] for agent in fleet}
    events = [event for event in client.events_seen[start:] if event.get("agent_id") in identifiers]
    if case.get("first_turn_children") is not None:
        errors.extend(first_turn_errors(events, root["id"], case["first_turn_children"]))
    errors.extend(event_requirement_errors(case, events, root["id"]))
    failures = [event["kind"] for event in events if tool_failed(event["kind"])]
    if failures:
        errors.append(f"{len(failures)} failed tool calls")
    tool_names = {event["kind"]["call"]["id"]: event["kind"]["call"]["name"]
                  for event in events if event["kind"]["type"] == "tool.requested"}
    tool_results = [{"name": tool_names.get(event["kind"]["call"], "unknown"),
                     "call": event["kind"]["call"],
                     "duration_ms": event["kind"]["outcome"]["duration_ms"],
                     "is_error": event["kind"]["outcome"].get("is_error", False)}
                    for event in events if event["kind"]["type"] == "tool.succeeded"]
    return {
        "name": case["name"], "prompt": case["prompt"], "model": case["model"],
        "root": root["id"], "passed": not errors, "errors": errors,
        "agents": fleet, "tool_failures": failures, "tool_results": tool_results,
        "first_turn_children": case.get("first_turn_children"),
        "lifecycle": [event for event in events if event["kind"]["type"] in
                      ("agent.created", "agent.started", "agent.turn_completed", "agent.interrupted")],
        "messages": [event for event in events if event["kind"]["type"] == "agent.message"
                     and event["kind"]["role"] == "assistant"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/aether")
    parser.add_argument("--cases", default=str(Path(__file__).with_name("natural_language_cases.json")))
    parser.add_argument("--case", action="append", help="Case name; repeat to select several")
    parser.add_argument("--timeout", type=float, default=600)
    parser.add_argument("--repeat", type=int, default=1, help="Independent trials per selected case")
    parser.add_argument("--output", required=True)
    parser.add_argument(
        "--dry-run", action="store_true",
        help="Schema-validate the selected cases only: no daemon, no model call, no spend."
    )
    args = parser.parse_args()
    if args.repeat < 1:
        parser.error("--repeat must be positive")
    cases = json.loads(Path(args.cases).read_text())
    if args.case:
        unknown = set(args.case) - {case["name"] for case in cases}
        if unknown:
            parser.error(f"Unknown cases: {sorted(unknown)}")
        cases = [case for case in cases if case["name"] in args.case]
    if args.dry_run:
        results = []
        for case in cases:
            errors = validate_case_schema(case)
            results.append({"name": case_display_name(case), "passed": not errors, "errors": errors})
        output = Path(args.output)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps({"dry_run": True, "results": results}, indent=2) + "\n")
        for result in results:
            print(json.dumps(result), flush=True)
        raise SystemExit(0 if all(r["passed"] for r in results) else 1)
    binary = str(Path(args.binary).resolve())
    if args.repeat > 1:
        cases = [{**case, "name": f"{case['name']}/trial-{trial}"}
                 for case in cases for trial in range(1, args.repeat + 1)]
    report = {"started_at": datetime.now(timezone.utc).isoformat(),
              "binary_sha256": hashlib.sha256(Path(binary).read_bytes()).hexdigest(), "results": []}
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    for case in cases:
        # A failed tree cannot affect scheduling or events in the next case.
        with tempfile.TemporaryDirectory(prefix="aether-natural-") as directory:
            env = isolated_env(directory)
            workspace = Path(directory) / "workspace"
            workspace.mkdir()
            (workspace / "README.md").write_text("# Documentation fixture\n\nThis project demonstrates tracked delegation.\n")
            try:
                init_git_workspace(workspace)
                start_daemon(binary, env, str(workspace))
                with AetherClient(env["AETHER_SOCKET"]) as client:
                    client.subscribe()
                    result = check(client, case, args.timeout)
                report["results"].append(result)
                print(json.dumps({key: result[key] for key in ("name", "passed", "errors")}), flush=True)
            except Exception as error:
                report["results"].append({"name": case["name"], "passed": False, "error": str(error)})
                raise
            finally:
                output.write_text(json.dumps(report, indent=2) + "\n")
                stop_daemon(binary, env)
    raise SystemExit(0 if all(result["passed"] for result in report["results"]) else 1)


if __name__ == "__main__":
    main()
