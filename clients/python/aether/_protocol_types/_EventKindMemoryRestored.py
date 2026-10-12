# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindMemoryRestored(TypedDict, total=False):
    """
    Put back after a `memory.forgotten`, so a mis-keyed curation has a way
    back that leaves the same trail everything else does.
    """
    memory: _protocol.MemoryId
    type: Literal["memory.restored"]
