# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class PendingArtifactCleanup(TypedDict, total=False):
    pending_unlinks: list[_protocol.ArtifactId]
    staged_publications: list[_protocol.ArtifactId]
