# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Event(TypedDict, total=False):
    """
    A committed event.
    """
    actor: _protocol.Actor
    agent_id: _protocol.AgentId | None  # optional
    at: str
    causation: _protocol.EventId | None  # optional
    id: _protocol.EventId
    kind: _protocol.EventKind
    parent_agent_id: _protocol.AgentId | None  # optional
    run: _protocol.RunId | None  # optional
    seq: int
    task_id: _protocol.TaskId | None  # optional
