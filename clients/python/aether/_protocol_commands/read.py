# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    ArtifactId,
)

def read_artifact(*, id: ArtifactId, max_bytes: int) -> dict[str, Any]:
    """Bytes, base64-encoded, capped at `max_bytes`."""
    frame: dict[str, Any] = {"cmd": "read_artifact"}
    frame["id"] = id
    frame["max_bytes"] = max_bytes
    return frame
