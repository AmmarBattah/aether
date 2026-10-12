# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
)

def providers() -> dict[str, Any]:
    """Every provider the registry knows, with its live health."""
    frame: dict[str, Any] = {"cmd": "providers"}
    return frame
