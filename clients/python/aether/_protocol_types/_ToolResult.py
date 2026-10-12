# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._ToolResultCancelled import ToolResultCancelled
from ._ToolResultDenied import ToolResultDenied
from ._ToolResultFailed import ToolResultFailed
from ._ToolResultPending import ToolResultPending
from ._ToolResultReplayed import ToolResultReplayed
from ._ToolResultSucceeded import ToolResultSucceeded


ToolResult = (
    ToolResultPending |
    ToolResultSucceeded |
    ToolResultFailed |
    ToolResultDenied |
    ToolResultCancelled |
    ToolResultReplayed
)
