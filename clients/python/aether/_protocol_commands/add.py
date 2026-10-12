# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    ApprovalDecision,
    RuleMatch,
    RuleScope,
)

def add_approval_rule(*, decision: ApprovalDecision, match: RuleMatch, scope: RuleScope, note: str | None = None) -> dict[str, Any]:
    """A remembered decision written directly, without answering a call first."""
    frame: dict[str, Any] = {"cmd": "add_approval_rule"}
    frame["decision"] = decision
    frame["match"] = match
    if note is not None:
        frame["note"] = note
    frame["scope"] = scope
    return frame
