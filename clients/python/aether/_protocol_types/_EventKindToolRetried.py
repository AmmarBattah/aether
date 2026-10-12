# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindToolRetried(TypedDict, total=False):
    """
    A retryable failure is being tried again, within the class's policy.
    """
    attempt: int
    call: _protocol.ToolCallId
    error: str
    of: int
    type: Literal["tool.retried"]
