# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindArtifactEvicted(TypedDict, total=False):
    """
    Retention dropped the bytes. The record stays; the payload is gone.
    """
    artifact: _protocol.ArtifactId
    reason: str
    size_bytes: int
    type: Literal["artifact.evicted"]
