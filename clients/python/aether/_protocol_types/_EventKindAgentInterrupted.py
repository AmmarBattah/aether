# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindAgentInterrupted(TypedDict, total=False):
    """
    The operator stopped the running turn. The agent keeps its
    transcript and waits for the next message.
    """
    turn: int
    type: Literal["agent.interrupted"]
