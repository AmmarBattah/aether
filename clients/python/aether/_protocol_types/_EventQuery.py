# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventQuery(TypedDict, total=False):
    agent: _protocol.AgentId | None  # optional
    before: int | None  # optional
    families: list[str]  # optional
    run: _protocol.RunId | None  # optional
    task: _protocol.TaskId | None  # optional
    types: list[str]  # optional
