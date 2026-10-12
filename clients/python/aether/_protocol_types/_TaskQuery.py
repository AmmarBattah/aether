# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class TaskQuery(TypedDict, total=False):
    owner: _protocol.AgentId | None  # optional
    run: _protocol.RunId | None  # optional
    statuses: list[_protocol.TaskStatusKind]  # optional
