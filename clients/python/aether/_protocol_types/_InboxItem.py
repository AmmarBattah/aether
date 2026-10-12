# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class InboxItem(TypedDict, total=False):
    """
    One delivered item. `seq` is per-agent and strictly increasing.
    """
    agent_id: _protocol.AgentId
    delivered_at: str
    id: _protocol.InboxItemId
    kind: _protocol.InboxKind
    seq: int
