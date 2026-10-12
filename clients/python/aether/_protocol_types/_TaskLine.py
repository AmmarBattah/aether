# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class TaskLine(TypedDict, total=False):
    created_at: str
    dependencies: list[_protocol.TaskId]  # optional
    ended_at: str | None  # optional
    id: _protocol.TaskId
    objective: str
    outcome: _protocol.Ended
    owner: _protocol.AgentId | None  # optional
    parent_task: _protocol.TaskId | None  # optional
    priority: _protocol.Priority
