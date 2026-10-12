# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def restart(*, agent: AgentId) -> dict[str, Any]:
    """Recreate a failed or cancelled agent from its last checkpoint, as a
    new agent linked to it.
    """
    frame: dict[str, Any] = {"cmd": "restart"}
    frame["agent"] = agent
    return frame
