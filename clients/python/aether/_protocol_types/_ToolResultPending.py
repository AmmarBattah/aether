# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# How a tool call in this turn ended.
# Tagged by `result`.
class ToolResultPending(TypedDict, total=False):
    """
    Requested, and the log carries no end for it.
    """
    result: Literal["pending"]
