# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ToolResultReplayed(TypedDict, total=False):
    """
    Answered from a committed outcome; the call did not run again.
    """
    original: _protocol.ToolCallId
    result: Literal["replayed"]
