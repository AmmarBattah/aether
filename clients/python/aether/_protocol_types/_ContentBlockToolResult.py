# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ContentBlockToolResult(TypedDict, total=False):
    content: list[_protocol.ToolResultBlock]
    is_error: bool  # optional
    tool_use_id: str
    type: Literal["tool_result"]
