# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindTaskCreated(TypedDict, total=False):
    dependencies: list[_protocol.TaskId]  # optional
    objective: str
    parent_task: _protocol.TaskId | None  # optional
    priority: _protocol.Priority
    type: Literal["task.created"]
