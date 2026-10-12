# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    MemoryQuerySpec,
)

def memories(*, query: MemoryQuerySpec) -> dict[str, Any]:
    """The `memories` command."""
    frame: dict[str, Any] = {"cmd": "memories"}
    frame["query"] = query
    return frame
