# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindMemoryPinned(TypedDict, total=False):
    """
    Pinned in front of the model, or unpinned.
    """
    memory: _protocol.MemoryId
    pinned: bool
    type: Literal["memory.pinned"]
