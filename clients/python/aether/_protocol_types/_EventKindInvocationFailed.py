# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindInvocationFailed(TypedDict, total=False):
    """
    The turn did not complete. Streamed content accumulated before the
    failure or cancellation is kept so nothing the user watched is lost.
    """
    cancelled: bool  # optional
    class_: _protocol.ModelErrorClass | None  # optional
    error: str
    invocation: _protocol.InvocationId
    partial: list[_protocol.ContentBlock]  # optional
    retryable: bool
    snapshot: _protocol.SnapshotId
    type: Literal["invocation.failed"]
