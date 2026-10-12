# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindAgentRestarted(TypedDict, total=False):
    """
    A failed or cancelled agent was recreated from its last checkpoint
    (§3.8). Recorded on both agents so either end of the link is visible
    from the other's event stream.
    """
    checkpoint: _protocol.CheckpointId | None  # optional
    dropped_deadline: str | None  # optional
    from_: _protocol.AgentId
    inbox_carried: int  # optional
    to: _protocol.AgentId
    transcript_dropped: int  # optional
    transcript_tail: int
    type: Literal["agent.restarted"]
