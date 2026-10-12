# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ScheduleOccurrence(TypedDict, total=False):
    """
    History describes delivery or spawning, never claims that a model
    completed the requested work. The ids link to the execution trace.
    """
    due: str
    outcome: _protocol.ScheduleOutcome
    recorded_at: str
    schedule: _protocol.ScheduleId
