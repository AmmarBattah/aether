# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindInvocationCompleted(TypedDict, total=False):
    duration_ms: int
    invocation: _protocol.InvocationId
    model: _protocol.ModelRef
    snapshot: _protocol.SnapshotId
    stop_reason: _protocol.StopReason | None  # optional
    type: Literal["invocation.completed"]
    usage: _protocol.Usage
