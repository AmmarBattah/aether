# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ToolCall(TypedDict, total=False):
    """
    One tool invocation requested by a model.
    """
    agent_id: _protocol.AgentId
    id: _protocol.ToolCallId
    idempotency_key: _protocol.IdempotencyKey | None  # optional
    input: Any
    invocation_id: _protocol.InvocationId | None  # optional
    name: str
    provider_call_id: str
    requested_at: str
    resources: list[_protocol.ResourceKey]  # optional
    task_id: _protocol.TaskId | None  # optional
