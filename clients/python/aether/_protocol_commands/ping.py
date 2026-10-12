# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
)

def ping() -> dict[str, Any]:
    """Keep an otherwise silent connection alive under an idle timeout.
    Answered with [`Reply::Ok`].
    """
    frame: dict[str, Any] = {"cmd": "ping"}
    return frame
