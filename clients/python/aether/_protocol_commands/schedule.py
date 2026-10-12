# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    ScheduleId,
)

def schedule_history(*, id: ScheduleId, limit: int) -> dict[str, Any]:
    """The `schedule_history` command."""
    frame: dict[str, Any] = {"cmd": "schedule_history"}
    frame["id"] = id
    frame["limit"] = limit
    return frame
