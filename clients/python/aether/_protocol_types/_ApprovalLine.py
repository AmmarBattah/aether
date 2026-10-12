# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ApprovalLine(TypedDict, total=False):
    decided_at: str | None  # optional
    decided_by: _protocol.Actor | None  # optional
    decision: _protocol.ApprovalDecision | None  # optional
    description: str
    id: _protocol.ApprovalId
    requested_at: str
    tool: str
