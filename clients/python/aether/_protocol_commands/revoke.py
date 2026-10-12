# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    ApprovalRuleId,
)

def revoke_approval_rule(*, id: ApprovalRuleId, reason: str | None = None) -> dict[str, Any]:
    """The `revoke_approval_rule` command."""
    frame: dict[str, Any] = {"cmd": "revoke_approval_rule"}
    frame["id"] = id
    if reason is not None:
        frame["reason"] = reason
    return frame
