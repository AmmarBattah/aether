# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindLeaseResourceAcquired(TypedDict, total=False):
    """
    A turn is holding resource keys for its mutating tool calls.
    """
    keys: list[_protocol.ResourceKey]
    type: Literal["lease.resource_acquired"]
