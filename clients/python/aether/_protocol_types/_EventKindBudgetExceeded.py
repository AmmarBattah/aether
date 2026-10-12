# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindBudgetExceeded(TypedDict, total=False):
    """
    A budget is spent; the agent stops as Failed with this reason.
    """
    reason: str
    type: Literal["budget.exceeded"]
    usage: _protocol.Usage
