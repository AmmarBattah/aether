# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def wake(*, agent: AgentId) -> dict[str, Any]:
    """Bring a failed agent back in place, with its history and context:
    it becomes Ready and reachable again under the same id. Additive in
    protocol 8; older daemons refuse the tag as unknown.
    """
    frame: dict[str, Any] = {"cmd": "wake"}
    frame["agent"] = agent
    return frame
