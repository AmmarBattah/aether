# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindLeaseResourceWaiting(TypedDict, total=False):
    """
    Another agent holds a key this turn needs; it waits, visibly Blocked.
    """
    blocked_on: _protocol.ResourceKey
    holder: _protocol.AgentId
    keys: list[_protocol.ResourceKey]
    type: Literal["lease.resource_waiting"]
