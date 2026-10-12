# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
)

def auth_status() -> dict[str, Any]:
    """The `auth_status` command."""
    frame: dict[str, Any] = {"cmd": "auth_status"}
    return frame
