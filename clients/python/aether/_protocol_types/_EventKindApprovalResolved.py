# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindApprovalResolved(TypedDict, total=False):
    approval: _protocol.ApprovalId
    by: _protocol.Actor
    decision: _protocol.ApprovalDecision
    note: str | None  # optional
    type: Literal["approval.resolved"]
