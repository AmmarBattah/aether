# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    AgentPatch,
    Any,
    ScheduleAction,
    ScheduleId,
)

def update_agent(*, id: AgentId, patch: AgentPatch) -> dict[str, Any]:
    """The `update_agent` command."""
    frame: dict[str, Any] = {"cmd": "update_agent"}
    frame["id"] = id
    frame["patch"] = patch
    return frame

def update_schedule(*, action: ScheduleAction, id: ScheduleId) -> dict[str, Any]:
    """The `update_schedule` command."""
    frame: dict[str, Any] = {"cmd": "update_schedule"}
    frame["action"] = action
    frame["id"] = id
    return frame
