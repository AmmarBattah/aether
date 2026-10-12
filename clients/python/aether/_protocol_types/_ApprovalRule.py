# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ApprovalRule(TypedDict, total=False):
    """
    A decision a person asked Aether to remember.
    """
    created_at: str
    created_by: _protocol.Actor
    decision: _protocol.ApprovalDecision
    id: _protocol.ApprovalRuleId
    match: _protocol.RuleMatch
    note: str | None  # optional
    revoked: _protocol.Revocation | None  # optional
    scope: _protocol.RuleScope
