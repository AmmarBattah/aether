# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    EventQuery,
)

def subscribe(*, deltas: bool, query: EventQuery, after: int | None = None) -> dict[str, Any]:
    """Stream events with `seq > after` (0 replays everything, `None` starts live)."""
    frame: dict[str, Any] = {"cmd": "subscribe"}
    if after is not None:
        frame["after"] = after
    frame["deltas"] = deltas
    frame["query"] = query
    return frame
