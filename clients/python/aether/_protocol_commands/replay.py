# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def replay(*, agent: AgentId) -> dict[str, Any]:
    """One agent's turns as they happened, folded from the log."""
    frame: dict[str, Any] = {"cmd": "replay"}
    frame["agent"] = agent
    return frame
