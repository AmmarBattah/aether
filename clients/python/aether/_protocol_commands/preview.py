# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
)

def preview_retention() -> dict[str, Any]:
    """Read-only retention decisions; never applies deletion or cleanup."""
    frame: dict[str, Any] = {"cmd": "preview_retention"}
    return frame
