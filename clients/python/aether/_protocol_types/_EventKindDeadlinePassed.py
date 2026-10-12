# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindDeadlinePassed(TypedDict, total=False):
    """
    The agent's wall-clock deadline passed; it stops as Failed.
    """
    reason: str
    type: Literal["deadline.passed"]
