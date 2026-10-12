# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def interrupt(*, agent: AgentId) -> dict[str, Any]:
    """Stop the running turn only; the agent keeps its transcript and waits
    for the next message.
    """
    frame: dict[str, Any] = {"cmd": "interrupt"}
    frame["agent"] = agent
    return frame
