# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class CreateScheduleSpec(TypedDict, total=False):
    """
    On the wire `target` and `spawn` are sibling fields, exactly one set, so
    a deliver request keeps the shape older clients send.
    """
    busy: _protocol.BusyPolicy  # optional
    max_deliveries: int | None  # optional
    missed: _protocol.MissedRunPolicy  # optional
    name: str
    payload: Any  # optional
    recurrence: _protocol.Recurrence
    spawn: _protocol.SpawnSpec | None  # optional
    target: _protocol.AgentId | None  # optional
