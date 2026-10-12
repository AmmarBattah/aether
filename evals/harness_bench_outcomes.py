"""Benchmark correctness, completion, staffing, and refusal classification."""
import time

from harness_bench_config import TEAM_CHILD_EFFORT
from harness_bench_wait import tree_is_quiescent


def base_trial_record(condition_key, case, run_index, short_id, trial_dir):
    return {
        "condition": condition_key, "case": case["id"], "run": run_index, "short_id": short_id,
        "trial_dir": trial_dir, "status": "preparing",
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }


def _overall_success(result):
    """Correctness and workflow completion are distinct: a timed-out or
    failed run does not count as successful even if the files it left
    behind would grade clean."""
    if any(result.get(key) for key in ("contract_violations", "protocol_violations",
                                       "harness_error", "cleanup_error", "daemon_cleanup_error",
                                       "integrity_violations", "cancel_error", "telemetry_error")):
        return False
    if result.get("workflow_status") != "completed":
        return False
    grade = result.get("grade")
    return bool(grade) and grade["total"] > 0 and grade["passed"] == grade["total"]


def _trial_status(result):
    """`blocked` (a host/environment refusal, see is_blocked_trial) takes
    precedence over `failed` so it is never folded into a correctness
    failure; a trial cannot be both."""
    if result.get("blocked"):
        return "blocked"
    return "completed" if result.get("success") else "failed"


def protocol_violations(condition, tree, root_id):
    nodes = tree["nodes"]
    descendants = set(nodes) - {root_id}
    direct = nodes[root_id]["children"]
    kind = condition["kind"]
    if kind == "aether-single":
        return ["solo condition created descendants"] if descendants else []
    violations = []
    if kind == "aether-coordinator-team" and (len(direct) != 2 or descendants != set(direct)):
        violations.append("team requires exactly two direct children and no grandchildren")
    expected = {"provider": "claude-code", "model": "sonnet"}
    for agent_id in sorted(descendants):
        agent = nodes[agent_id]["agent"]
        if agent["model"] != expected or agent.get("config", {}).get("effort") != TEAM_CHILD_EFFORT:
            violations.append(f"{agent_id}: unexpected child staffing")
        if agent["status"]["state"] != "completed":
            violations.append(f"{agent_id}: child did not complete")
    return violations


def normalized_workflow_status(condition, status, error, reply, tree, root_id):
    if not tree_is_quiescent(tree, root_id):
        return "timeout"
    if (condition["kind"] == "aether-coordinator-default" and status == "waiting"
            and error is None and isinstance(reply, str) and reply.strip()
            and tree["nodes"][root_id]["agent"]["status"].get("condition", {}).get("on") == "inbox"):
        return "completed"
    return status


def _tool_error_before(events, root_id, before_seq):
    """A tool "error" is the outcome the agent actually saw: a tool call the
    host refused (`tool.succeeded` carrying `outcome.is_error`), or a
    transport-level `tool.failed`. Both precede the seq of the idle wait."""
    for event in events:
        if event.get("agent_id") != root_id or event["seq"] >= before_seq:
            continue
        kind = event["kind"]
        if kind["type"] == "tool.failed":
            return True
        if kind["type"] == "tool.succeeded" and kind.get("outcome", {}).get("is_error"):
            return True
    return False


def is_blocked_trial(events, result, root_id):
    """Distinguish a host/environment refusal from a model correctness
    failure: the root agent was assigned its task, never created a single
    child, and idled on its inbox only after a tool call came back as an
    error (e.g. `delegate` refused for a missing host dependency). That is
    not evidence the model solved the task incorrectly, so it must be kept
    out of `failed` and excluded from pass-rate means by default.
    """
    assert isinstance(events, list), "events must be the run's raw recorded event list"
    assert root_id, "blocked classification requires a known root agent id"
    if result.get("descendant_count") != 0:
        return False
    if not any(e.get("agent_id") == root_id and e["kind"]["type"] == "task.assigned" for e in events):
        return False
    idle_events = [e for e in events if e.get("agent_id") == root_id
                   and e["kind"]["type"] == "agent.waiting"
                   and e["kind"].get("condition", {}).get("on") == "inbox"]
    if not idle_events:
        return False
    return _tool_error_before(events, root_id, idle_events[-1]["seq"])
