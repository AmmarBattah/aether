# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ResourceHolding(TypedDict, total=False):
    """
    Resource keys one agent's mutating tool call holds for the length of that
    call. Two agents never hold conflicting keys at once (VISION §3.6 limit 5).
    """
    agent: _protocol.AgentId
    keys: list[_protocol.ResourceKey]
    quarantined: bool  # optional
    since: str
