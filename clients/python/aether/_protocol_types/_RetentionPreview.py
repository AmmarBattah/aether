# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class RetentionPreview(TypedDict, total=False):
    artifact_payload_bytes: int
    at: str
    eligible_artifacts: list[_protocol.ArtifactId]
    history_before: str
    history_payload_bytes: int
    pending_cleanup: _protocol.PendingArtifactCleanup
    runs: list[_protocol.RunRetentionPreview]
