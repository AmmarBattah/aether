# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindApprovalRuleRevoked(TypedDict, total=False):
    """
    The rule stops deciding anything from here on.
    """
    revocation: _protocol.Revocation
    rule: _protocol.ApprovalRuleId
    type: Literal["approval.rule_revoked"]
