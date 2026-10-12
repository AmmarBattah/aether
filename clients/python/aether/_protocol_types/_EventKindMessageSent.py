# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindMessageSent(TypedDict, total=False):
    content: list[_protocol.ContentBlock]
    from_: _protocol.Actor
    message: _protocol.MessageId
    to: _protocol.AgentId
    type: Literal["message.sent"]
