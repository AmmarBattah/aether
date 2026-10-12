# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Content allowed inside a tool result.
# Tagged by `type`.
class ToolResultBlockText(TypedDict, total=False):
    text: str
    type: Literal["text"]
