# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def cancel(*, agent: AgentId, reason: str | None = None) -> dict[str, Any]:
    """The `cancel` command."""
    frame: dict[str, Any] = {"cmd": "cancel"}
    frame["agent"] = agent
    if reason is not None:
        frame["reason"] = reason
    return frame
