# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def compact(*, agent: AgentId) -> dict[str, Any]:
    """Ask the runtime to compact the agent's context before its next turn."""
    frame: dict[str, Any] = {"cmd": "compact"}
    frame["agent"] = agent
    return frame
