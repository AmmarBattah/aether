# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindAgentPaused(TypedDict, total=False):
    """
    The operator paused the agent (VISION §3.1). Durable and orthogonal to
    the status: it takes no turns until resumed, and its inbox fills.
    """
    type: Literal["agent.paused"]
    via: _protocol.AgentId | None  # optional
