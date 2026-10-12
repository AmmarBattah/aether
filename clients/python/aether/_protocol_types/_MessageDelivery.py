# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class MessageDelivery(TypedDict, total=False):
    """
    One agent's copy of a fanned-out message.
    """
    agent: _protocol.AgentId
    message: _protocol.MessageId
