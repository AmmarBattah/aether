# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class CheckpointProvenance(TypedDict, total=False):
    """
    Where a checkpoint came from.
    """
    compactor_model: _protocol.ModelRef | None  # optional
    from_seq: int
    previous: _protocol.CheckpointId | None  # optional
    source_snapshot: _protocol.SnapshotId | None  # optional
    to_seq: int
    trigger: _protocol.CompactionTrigger | None  # optional
