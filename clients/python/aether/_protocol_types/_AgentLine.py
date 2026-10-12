# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class AgentLine(TypedDict, total=False):
    created_at: str
    depth: int
    ended_at: str | None  # optional
    id: _protocol.AgentId
    model: _protocol.ModelRef
    name: str
    outcome: _protocol.Ended
    parent: _protocol.AgentId | None  # optional
    role: _protocol.Role
    subtree_usage: _protocol.Usage
    tool_calls: int
    turns: int
    usage: _protocol.Usage
    workspace: str | None  # optional
