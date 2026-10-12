# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindProviderQuota(TypedDict, total=False):
    provider: str
    type: Literal["provider.quota"]
    usage: _protocol.ProviderUsage | None  # optional
    window: _protocol.QuotaWindow
