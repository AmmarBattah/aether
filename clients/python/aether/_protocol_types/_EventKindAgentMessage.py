# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindAgentMessage(TypedDict, total=False):
    """
    A transcript entry: what the model said, or what it was shown.
    """
    content: list[_protocol.ContentBlock]
    invocation: _protocol.InvocationId | None  # optional
    role: _protocol.MessageRole
    type: Literal["agent.message"]
