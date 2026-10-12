# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def trigger(*, agent: AgentId, source: str, idempotency_key: str | None = None, payload: Any | None = None) -> dict[str, Any]:
    """Deliver an external trigger (a webhook, a timer, a CI result) to an
    agent's inbox. It wakes a waiting agent like a person's message.
    """
    frame: dict[str, Any] = {"cmd": "trigger"}
    frame["agent"] = agent
    if idempotency_key is not None:
        frame["idempotency_key"] = idempotency_key
    if payload is not None:
        frame["payload"] = payload
    frame["source"] = source
    return frame
