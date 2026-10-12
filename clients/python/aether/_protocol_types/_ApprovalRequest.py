# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ApprovalRequest(TypedDict, total=False):
    """
    A request for a human (or supervising agent) to allow a tool call.
    """
    agent_id: _protocol.AgentId
    argument: str | None  # optional
    call_id: _protocol.ToolCallId
    description: str
    id: _protocol.ApprovalId
    requested_at: str
    resources: list[_protocol.ResourceKey]  # optional
    side_effect: _protocol.SideEffectClass
    status: _protocol.ApprovalStatus
    task_id: _protocol.TaskId | None  # optional
    tool: str
