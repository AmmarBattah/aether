# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def latest_checkpoint(*, agent: AgentId) -> dict[str, Any]:
    """The `latest_checkpoint` command."""
    frame: dict[str, Any] = {"cmd": "latest_checkpoint"}
    frame["agent"] = agent
    return frame
