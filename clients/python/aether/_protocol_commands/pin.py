# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    MemoryId,
)

def pin_memory(*, id: MemoryId, pinned: bool) -> dict[str, Any]:
    """Keep a memory in front of the model, or stop."""
    frame: dict[str, Any] = {"cmd": "pin_memory"}
    frame["id"] = id
    frame["pinned"] = pinned
    return frame
