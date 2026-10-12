# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Typed event payloads, grouped by family. The serialized `type` tag uses
# dotted names (`agent.created`) so filters can match by family prefix.
# Tagged by `type`.
class EventKindAgentCreated(TypedDict, total=False):
    fallbacks: list[_protocol.ModelRef]  # optional
    model: _protocol.ModelRef
    name: str
    parent: _protocol.AgentId | None  # optional
    role: _protocol.Role
    scope: _protocol.Scope
    type: Literal["agent.created"]
