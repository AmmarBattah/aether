# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindMemoryForgotten(TypedDict, total=False):
    """
    Curated out by a person. The record stays; it is never retrieved again.
    """
    memory: _protocol.MemoryId
    reason: str | None  # optional
    type: Literal["memory.forgotten"]
