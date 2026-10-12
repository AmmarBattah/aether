# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindToolReplayed(TypedDict, total=False):
    """
    The call carried an idempotency key already committed for this agent,
    so the recorded outcome was returned instead of running again.
    """
    call: _protocol.ToolCallId
    key: _protocol.IdempotencyKey
    original: _protocol.ToolCallId
    type: Literal["tool.replayed"]
