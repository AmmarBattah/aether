# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
    ArtifactId,
    CheckpointId,
    EventQuery,
    InvocationId,
    RunId,
    ScheduleId,
    SnapshotId,
    TaskId,
)

def get_agent(*, id: AgentId) -> dict[str, Any]:
    """The `get_agent` command."""
    frame: dict[str, Any] = {"cmd": "get_agent"}
    frame["id"] = id
    return frame

def get_artifact(*, id: ArtifactId) -> dict[str, Any]:
    """The `get_artifact` command."""
    frame: dict[str, Any] = {"cmd": "get_artifact"}
    frame["id"] = id
    return frame

def get_checkpoint(*, id: CheckpointId) -> dict[str, Any]:
    """The `get_checkpoint` command."""
    frame: dict[str, Any] = {"cmd": "get_checkpoint"}
    frame["id"] = id
    return frame

def get_events(*, after: int, limit: int, query: EventQuery) -> dict[str, Any]:
    """The `get_events` command."""
    frame: dict[str, Any] = {"cmd": "get_events"}
    frame["after"] = after
    frame["limit"] = limit
    frame["query"] = query
    return frame

def get_github_watcher() -> dict[str, Any]:
    """Inspect the opt-in GitHub watcher and its per-repository health."""
    frame: dict[str, Any] = {"cmd": "get_github_watcher"}
    return frame

def get_inbox(*, after: int, agent: AgentId) -> dict[str, Any]:
    """The `get_inbox` command."""
    frame: dict[str, Any] = {"cmd": "get_inbox"}
    frame["after"] = after
    frame["agent"] = agent
    return frame

def get_invocation(*, id: InvocationId) -> dict[str, Any]:
    """The `get_invocation` command."""
    frame: dict[str, Any] = {"cmd": "get_invocation"}
    frame["id"] = id
    return frame

def get_run_report(*, run: RunId) -> dict[str, Any]:
    """Compose a run's report from its events."""
    frame: dict[str, Any] = {"cmd": "get_run_report"}
    frame["run"] = run
    return frame

def get_schedule(*, id: ScheduleId) -> dict[str, Any]:
    """The `get_schedule` command."""
    frame: dict[str, Any] = {"cmd": "get_schedule"}
    frame["id"] = id
    return frame

def get_snapshot(*, id: SnapshotId) -> dict[str, Any]:
    """The `get_snapshot` command."""
    frame: dict[str, Any] = {"cmd": "get_snapshot"}
    frame["id"] = id
    return frame

def get_task(*, id: TaskId) -> dict[str, Any]:
    """The `get_task` command."""
    frame: dict[str, Any] = {"cmd": "get_task"}
    frame["id"] = id
    return frame

def get_transcript(*, after: int, agent: AgentId) -> dict[str, Any]:
    """The `get_transcript` command."""
    frame: dict[str, Any] = {"cmd": "get_transcript"}
    frame["after"] = after
    frame["agent"] = agent
    return frame

def get_tree(*, include_archived: bool | None = None, run: RunId | None = None) -> dict[str, Any]:
    """The live graph: every agent in the run (or all runs) with relations."""
    frame: dict[str, Any] = {"cmd": "get_tree"}
    if include_archived is not None:
        frame["include_archived"] = include_archived
    if run is not None:
        frame["run"] = run
    return frame
