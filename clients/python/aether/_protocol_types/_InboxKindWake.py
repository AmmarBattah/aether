# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class InboxKindWake(TypedDict, total=False):
    """
    A bare wake with a reason (deadline passed, operator nudge).
    """
    kind: Literal["wake"]
    reason: str
