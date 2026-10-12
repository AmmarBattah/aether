# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Why the model stopped generating.
# Tagged by `reason`.
class StopReasonEndTurn(TypedDict, total=False):
    reason: Literal["end_turn"]
