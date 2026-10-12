"""Benchmark event waits and whole-run quiescence checks."""
import time

from aether_client import protocol

TERMINAL_STATES = {"completed", "failed", "cancelled"}


def wait_for_run_terminal(client, agent_id, timeout):
    """Drive events for a single-shot (`long_lived=False`) agent until it
    completes, fails, is cancelled, or stalls idle before finishing."""
    assert timeout > 0, "a run-style wait needs a positive timeout"
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            frame = client.next_frame(timeout=max(0.01, deadline - time.monotonic()))
        except TimeoutError:
            break
        if frame.get("frame") != "event" or frame["event"].get("agent_id") != agent_id:
            continue
        kind = frame["event"]["kind"]
        if kind["type"] == "agent.completed":
            return "completed", None, None
        if kind["type"] == "agent.failed":
            return "failed", kind.get("error") or "agent failed", None
        if kind["type"] == "agent.cancelled":
            return "cancelled", kind.get("reason") or "agent cancelled", None
        if kind["type"] == "agent.waiting" and kind.get("condition", {}).get("on") == "inbox":
            return "waiting", "agent went idle before completing the objective", None
    return "timeout", f"no terminal event within {timeout:.0f}s", None


def wait_for_turn_reply(client, agent_id, timeout):
    """Drive a long-lived agent's current turn to its own idle point, in the
    same 3-tuple shape as wait_for_run_terminal so callers share one path."""
    assert timeout > 0, "a reply wait needs a positive timeout"
    try:
        return "waiting", None, client.wait_for_reply(agent_id, timeout=timeout)
    except TimeoutError as error:
        return "timeout", str(error), None
    except RuntimeError as error:
        return "failed", str(error), None


def fetch_tree(client, run_id, timeout=5):
    return client.request(protocol.get_tree(run=run_id), timeout=timeout)["data"]


def tree_is_quiescent(tree, root_id):
    """True once every node has reached a terminal state, or - for the root
    of a long-lived coordinator - has gone idle waiting on its inbox."""
    nodes = tree.get("nodes", {})
    assert nodes, "get_tree returned an empty graph for a run that has a root"
    assert root_id in nodes, f"root {root_id} missing from its own run's tree"
    for agent_id, node in nodes.items():
        state = node["agent"]["status"]["state"]
        if state in TERMINAL_STATES:
            continue
        if (agent_id == root_id and state == "waiting"
                and node["agent"]["status"].get("condition", {}).get("on") == "inbox"):
            continue
        return False
    return True


def wait_for_tree_quiescent(client, run_id, root_id, timeout):
    """Poll get_tree until every descendant this run ever created has
    finished, so a coordinator's own completion is never trusted alone."""
    assert timeout > 0, "a quiescence wait needs a positive timeout"
    deadline = time.monotonic() + timeout
    tree = fetch_tree(client, run_id, timeout=min(5, timeout))
    while not tree_is_quiescent(tree, root_id):
        if time.monotonic() >= deadline:
            return tree, False
        time.sleep(min(2.0, max(0.0, deadline - time.monotonic())))
        tree = fetch_tree(client, run_id, timeout=max(0.01, min(5, deadline - time.monotonic())))
    return tree, True


def collect_run_events(client, run_id, page_limit=5000):
    """Page through every durable event for `run_id`, oldest first."""
    assert 0 < page_limit <= 5000, "get_events documents a 5000-event page cap"
    events, after = [], 0
    while True:
        page = client.request(protocol.get_events(after=after, limit=page_limit, query={"run": run_id}))["data"]
        if not page:
            break
        events.extend(page)
        after = max(event["seq"] for event in page)
        if len(page) < page_limit:
            break
    return events


def create_agent_named(client, *, role, name, model, effort, long_lived):
    """client.create_agent() (aether_client.py, not edited here) has no
    `name`; the default-coordinator condition needs one to look like a real
    TUI-created root, so this calls protocol.create_agent directly."""
    provider, model_name = model.split("/", 1)
    config = {"long_lived": long_lived, "approval": {"require_for": []}, "effort": effort}
    return client.request(protocol.create_agent(
        role=role, start=True, name=name, model={"provider": provider, "model": model_name}, config=config,
    ))["data"]
