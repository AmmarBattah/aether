# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    MemoryId,
)

def restore_memory(*, id: MemoryId) -> dict[str, Any]:
    """Put a forgotten memory back. The inverse of `ForgetMemory`, so a
    mis-keyed curation has a way back.
    """
    frame: dict[str, Any] = {"cmd": "restore_memory"}
    frame["id"] = id
    return frame
