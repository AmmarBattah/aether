# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._StopReasonEndTurn import StopReasonEndTurn
from ._StopReasonMaxTokens import StopReasonMaxTokens
from ._StopReasonOther import StopReasonOther
from ._StopReasonPauseTurn import StopReasonPauseTurn
from ._StopReasonRefusal import StopReasonRefusal
from ._StopReasonStopSequence import StopReasonStopSequence
from ._StopReasonToolUse import StopReasonToolUse


StopReason = (
    StopReasonEndTurn |
    StopReasonToolUse |
    StopReasonMaxTokens |
    StopReasonStopSequence |
    StopReasonRefusal |
    StopReasonPauseTurn |
    StopReasonOther
)
