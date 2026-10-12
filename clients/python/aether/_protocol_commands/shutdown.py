# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
)

def shutdown() -> dict[str, Any]:
    """The `shutdown` command."""
    frame: dict[str, Any] = {"cmd": "shutdown"}
    return frame
