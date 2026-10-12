# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ProviderHealth(TypedDict, total=False):
    """
    One provider's scheduling picture. Derived state: the scheduler rebuilds
    it from the turns in flight and the feedback providers have given.
    """
    backoff: _protocol.ProviderBackoff | None  # optional
    last_fallback: _protocol.FallbackRecord | None  # optional
    limit: int | None  # optional
    provider: str
    quota: _protocol.QuotaWindow | None  # optional
    recent_errors: list[_protocol.ErrorTally]  # optional
    running: int
    suspended_agents: int  # optional
    usage: _protocol.ProviderUsage | None  # optional
