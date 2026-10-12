# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Lease(TypedDict, total=False):
    agent_id: _protocol.AgentId
    expires_at: str
    granted_at: str
    holder: str
    turn: int
