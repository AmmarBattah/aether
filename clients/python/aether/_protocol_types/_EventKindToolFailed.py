# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindToolFailed(TypedDict, total=False):
    call: _protocol.ToolCallId
    duration_ms: int | None  # optional
    error: str
    retryable: bool
    type: Literal["tool.failed"]
