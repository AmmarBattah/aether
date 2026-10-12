# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindBudgetWarning(TypedDict, total=False):
    """
    A budget is 80 % spent: the person is told before the stop.
    """
    reason: str
    type: Literal["budget.warning"]
    usage: _protocol.Usage
