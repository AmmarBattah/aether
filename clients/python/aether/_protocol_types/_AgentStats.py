# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class AgentStats(TypedDict, total=False):
    """
    Lifetime token counters and activity stats.
    """
    context_tokens: int | None  # optional
    last_turn_at: str | None  # optional
    tool_calls: int  # optional
    turns: int  # optional
    usage: _protocol.Usage  # optional
