# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ApprovalStatusResolved(TypedDict, total=False):
    at: str
    by: _protocol.Actor
    decision: _protocol.ApprovalDecision
    note: str | None  # optional
    state: Literal["resolved"]
