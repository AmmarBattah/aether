# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindApprovalRuleAdded(TypedDict, total=False):
    """
    A decision was remembered as a durable rule.
    """
    rule: _protocol.ApprovalRule
    type: Literal["approval.rule_added"]
