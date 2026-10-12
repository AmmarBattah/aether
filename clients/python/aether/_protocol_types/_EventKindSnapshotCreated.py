# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindSnapshotCreated(TypedDict, total=False):
    layers: list[_protocol.ContextLayer]
    model: _protocol.ModelRef
    snapshot: _protocol.SnapshotId
    token_estimate: int
    turn: int
    type: Literal["snapshot.created"]
