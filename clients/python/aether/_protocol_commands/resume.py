# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def resume(*, agent: AgentId, subtree: bool | None = None) -> dict[str, Any]:
    """Lift a pause, or clear a wait or an inspection the operator owns.
    With `subtree` it lifts the pause across the subtree, and only that.
    """
    frame: dict[str, Any] = {"cmd": "resume"}
    frame["agent"] = agent
    if subtree is not None:
        frame["subtree"] = subtree
    return frame
