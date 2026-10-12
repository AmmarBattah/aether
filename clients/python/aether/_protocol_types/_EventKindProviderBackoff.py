# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindProviderBackoff(TypedDict, total=False):
    """
    The scheduler is holding turns back from a provider: a 429, a
    repeated failure, or a subscription window near its limit.
    """
    provider: str
    reason: str
    type: Literal["provider.backoff"]
    until: str
