# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    MemoryId,
)

def forget_memory(*, id: MemoryId, reason: str | None = None) -> dict[str, Any]:
    """Curate a memory out. The record stays; it is never retrieved again."""
    frame: dict[str, Any] = {"cmd": "forget_memory"}
    frame["id"] = id
    if reason is not None:
        frame["reason"] = reason
    return frame
