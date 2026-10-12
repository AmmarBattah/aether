# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class CreateTask(TypedDict, total=False):
    dependencies: list[_protocol.TaskId]  # optional
    idempotency_key: str | None  # optional
    objective: str
    owner: _protocol.AgentId | None  # optional
    parent_task: _protocol.TaskId | None  # optional
    priority: _protocol.Priority  # optional
