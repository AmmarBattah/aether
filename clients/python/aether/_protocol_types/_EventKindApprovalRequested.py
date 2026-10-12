# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindApprovalRequested(TypedDict, total=False):
    approval: _protocol.ApprovalId
    argument: str | None  # optional
    call: _protocol.ToolCallId
    description: str
    side_effect: _protocol.SideEffectClass
    tool: str
    type: Literal["approval.requested"]
