# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindCompactionCompleted(TypedDict, total=False):
    after_tokens: int | None  # optional
    before_tokens: int | None  # optional
    checkpoint: _protocol.CheckpointId
    from_seq: int
    to_seq: int
    type: Literal["compaction.completed"]
    version: int
