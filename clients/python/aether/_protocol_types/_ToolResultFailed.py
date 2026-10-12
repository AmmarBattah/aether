# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ToolResultFailed(TypedDict, total=False):
    duration_ms: int | None  # optional
    error: str
    result: Literal["failed"]
    retryable: bool
