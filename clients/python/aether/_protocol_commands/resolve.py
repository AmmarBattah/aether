# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    ApprovalDecision,
    ApprovalId,
    RememberSpec,
)

def resolve_approval(*, decision: ApprovalDecision, id: ApprovalId, note: str | None = None, remember: RememberSpec | None = None) -> dict[str, Any]:
    """The `resolve_approval` command."""
    frame: dict[str, Any] = {"cmd": "resolve_approval"}
    frame["decision"] = decision
    frame["id"] = id
    if note is not None:
        frame["note"] = note
    if remember is not None:
        frame["remember"] = remember
    return frame
