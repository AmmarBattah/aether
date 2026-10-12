# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ReplyMessagesSent(TypedDict, total=False):
    """
    One entry per agent a fanned-out message reached, in delivery order.
    """
    data: dict[str, Any]
    reply: Literal["messages_sent"]
