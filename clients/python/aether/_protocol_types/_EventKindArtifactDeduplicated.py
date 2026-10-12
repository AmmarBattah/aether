# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindArtifactDeduplicated(TypedDict, total=False):
    """
    An identical artifact already existed, so the bytes were not stored
    again and the call was given the existing one.
    """
    artifact: _protocol.ArtifactId
    digest: str
    shared_with: _protocol.ArtifactId | None  # optional
    type: Literal["artifact.deduplicated"]
