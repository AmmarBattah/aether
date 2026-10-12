"""Deadline wakeup checks, separate from child completion and cancellation."""
from datetime import datetime
import time

import harness_bench as hb
from orchestration_worker import track_worker, worker_exited


WAIT_SECONDS = 40


def deadlines(condition):
    if condition["on"] == "deadline":
        return [condition["at"]]
    return [at for child in condition.get("of", []) for at in deadlines(child)]


def assess_timeout(events, root, child, child_task, parked):
    errors = []
    dates = deadlines(parked["kind"]["condition"])
    if len(dates) != 1:
        return ["expected exactly one persisted wait deadline"]
    resumes = [event for event in events if event.get("agent_id") == root
               and event["seq"] > parked["seq"] and event["kind"]["type"] == "agent.started"]
    if len(resumes) != 1:
        return ["expected exactly one parent deadline resumption"]
    resume = resumes[0]
    if datetime.fromisoformat(resume["at"]) < datetime.fromisoformat(dates[0]):
        errors.append("parent resumed before its persisted deadline")
    if any(event.get("agent_id") == child and event["seq"] <= resume["seq"]
           and event["kind"]["type"] in ("agent.completed", "agent.failed", "agent.cancelled")
           for event in events):
        errors.append("child became terminal before the deadline wakeup")
    cancelled = [event for event in events if event.get("agent_id") == child
                 and event["kind"]["type"] == "agent.cancelled"]
    if len(cancelled) != 1 or cancelled[0]["seq"] <= resume["seq"]:
        errors.append("child was not cancelled exactly once after parent deadline resumption")
    calls = [event for event in events if event.get("agent_id") == root
             and event["kind"]["type"] == "tool.requested"]
    waits = [event for event in calls if event["kind"]["call"]["name"].split("__")[-1] == "wait_for"]
    if (len(waits) != 1
            or waits[0]["kind"]["call"]["input"].get("timeout_seconds") != WAIT_SECONDS
            or waits[0]["kind"]["call"]["input"].get("tasks") != [child_task]):
        errors.append("parent did not use exactly one requested timed wait")
    cancels = [event for event in calls if event["kind"]["call"]["name"].split("__")[-1] == "cancel_agent"]
    if (len(cancels) != 1 or cancels[0]["seq"] <= resume["seq"]
            or cancels[0]["kind"]["call"]["input"].get("agent") != child):
        errors.append("parent did not explicitly cancel its child after the deadline wakeup")
    return errors


def finish_timeout(client, root, run, parked, workspace, child_model, deadline):
    tree = hb.fetch_tree(client, run)
    children = [node["agent"] for aid, node in tree["nodes"].items() if aid != root]
    provider, model = child_model.split("/", 1)
    if (len(children) != 1 or children[0]["parent"] != root
            or children[0]["model"] != {"provider": provider, "model": model}
            or children[0]["config"].get("effort") != "low"
            or children[0]["status"]["state"] != "running"):
        raise RuntimeError("timeout requires one running child with the requested model and effort")
    child = children[0]["id"]
    child_task = children[0]["current_task"]
    if child_task is None:
        raise RuntimeError("timeout child has no current task")
    errors = []
    with track_worker(workspace) as worker:
        status, error, _ = hb.wait_for_run_terminal(client, root, max(0.01, deadline - time.monotonic()))
        if status != "completed":
            errors.append(error or f"timeout parent ended {status}")
        tree, quiescent = hb.wait_for_tree_quiescent(client, run, root, 15)
        exited = worker_exited(worker)
    if not quiescent or tree["nodes"][child]["agent"]["status"]["state"] != "cancelled":
        errors.append("child did not settle cancelled after timeout")
    if not exited:
        errors.append("worker survived timeout cleanup")
    if (workspace / "release.json").exists() or (workspace / "answer.json").exists():
        errors.append("timeout fixture gate was released or answer was manufactured")
    events = hb.collect_run_events(client, run)
    errors.extend(assess_timeout(events, root, child, child_task, parked))
    replies = [block.get("text", "") for event in events if event.get("agent_id") == root
               and event["kind"]["type"] == "agent.message" and event["kind"].get("role") == "assistant"
               for block in event["kind"]["content"] if block.get("type") == "text"]
    if not replies or "TIMED_OUT" not in replies[-1]:
        errors.append("parent did not report TIMED_OUT")
    return {"errors": errors, "worker_exited": exited, "workflow_status": status}, events
