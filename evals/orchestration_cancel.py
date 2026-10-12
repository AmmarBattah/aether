"""Cancellation-specific checks for the mixed-native wait smoke.

These checks establish durable subtree cancellation and no parent model work.
They also track the gated worker's exit, not the entire OS process tree or billing.
"""
import time

from aether_client import protocol
import harness_bench as hb
from orchestration_worker import track_worker, worker_exited


def assess_cancel(events, root, park_seq, cancel_seq, before, after, tree):
    errors = []
    if before != after:
        errors.append("parent counters changed between parking and cancellation settlement")
    nodes = tree["nodes"]
    if root not in nodes or len(nodes) != 2:
        errors.append("expected exactly the parent and one child in cancelled tree")
    for agent_id, node in nodes.items():
        if node["agent"]["status"]["state"] != "cancelled":
            errors.append(f"agent {agent_id} was not cancelled")
        cancellations = [event for event in events if event.get("agent_id") == agent_id
                         and event["seq"] > cancel_seq
                         and event["kind"]["type"] == "agent.cancelled"]
        if len(cancellations) != 1:
            errors.append(f"expected exactly one post-request cancellation for {agent_id}")
    forbidden = {"agent.started", "snapshot.created", "agent.usage_updated", "tool.requested"}
    if any(event.get("agent_id") == root and event["seq"] > park_seq
           and event["kind"]["type"] in forbidden for event in events):
        errors.append("parent performed model work after parking during subtree cancellation")
    return errors


def cancel_parked_tree(client, root, run, parked, before, child_model, workspace, observe):
    """Cancel once; consume events during a bounded post-cancellation observation."""
    tree = hb.fetch_tree(client, run)
    children = [node["agent"] for aid, node in tree["nodes"].items() if aid != root]
    provider, model = child_model.split("/", 1)
    if (len(children) != 1 or children[0]["parent"] != root
            or children[0]["model"] != {"provider": provider, "model": model}
            or children[0]["config"].get("effort") != "low"
            or children[0]["status"]["state"] != "running"):
        raise RuntimeError("cancellation requires one running child with requested model and effort")
    if not (workspace / "worker.started").exists():
        raise RuntimeError("child had not started the gated worker before cancellation")
    events = hb.collect_run_events(client, run)
    cancel_seq = max(event["seq"] for event in events)
    started = time.monotonic()
    with track_worker(workspace) as worker:
        client.request(protocol.cancel(agent=root, reason="isolated orchestration cancellation test"),
                       timeout=10)
        # This is an external observer's bounded state check, not a model polling loop.
        tree, quiescent = hb.wait_for_tree_quiescent(client, run, root, 15)
        settlement_seconds = time.monotonic() - started
        observe(client, 5)
        exited = worker_exited(worker)
    tree = hb.fetch_tree(client, run)
    after_agent = client.get_agent(root)
    after = {key: after_agent["stats"][key] for key in ("turns", "usage", "tool_calls")}
    events = hb.collect_run_events(client, run)
    errors = assess_cancel(events, root, parked["seq"], cancel_seq, before, after, tree)
    if not quiescent:
        errors.append("cancelled subtree did not become quiescent within 15 seconds")
    if not exited:
        errors.append("gated worker process survived subtree cancellation")
    if (workspace / "release.json").exists() or (workspace / "answer.json").exists():
        errors.append("fixture gate was released or child produced an answer during cancellation")
    return {"cancel_seq": cancel_seq, "settlement_seconds": settlement_seconds,
            "worker_exited": exited,
            "post_cancel_observation_seconds": 5, "before": before, "after": after,
            "states": {aid: node["agent"]["status"]["state"] for aid, node in tree["nodes"].items()},
            "errors": errors,
            "limitation": "Tracks one gated worker, not every OS descendant; interrupted usage may be incomplete."}
