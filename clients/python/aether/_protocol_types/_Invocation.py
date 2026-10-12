# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Invocation(TypedDict, total=False):
    """
    The result of sending a snapshot to a model.
    """
    agent_id: _protocol.AgentId
    content: list[_protocol.ContentBlock]  # optional
    error: str | None  # optional
    finished_at: str | None  # optional
    id: _protocol.InvocationId
    model: _protocol.ModelRef
    provider_message_id: str | None  # optional
    snapshot_id: _protocol.SnapshotId
    started_at: str
    stop_reason: _protocol.StopReason | None  # optional
    turn: int
    usage: _protocol.Usage  # optional
