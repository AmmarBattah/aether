# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Schedule(TypedDict, total=False):
    """
    Timers enqueue work, never authority: targets keep their scope and budgets;
    spawned runs get only what an operator could grant via `create_agent`.
    """
    busy: _protocol.BusyPolicy  # optional
    created_at: str
    deliveries: int
    id: _protocol.ScheduleId
    last: _protocol.ScheduleOccurrence | None  # optional
    last_inbox_seq: int | None  # optional
    last_spawned: _protocol.AgentId | None  # optional
    max_deliveries: int | None  # optional
    missed: _protocol.MissedRunPolicy
    name: str
    owner: _protocol.Actor
    payload: Any
    recurrence: _protocol.Recurrence
    run: _protocol.RunId | None  # optional
    spawn: _protocol.SpawnSpec | None  # optional
    status: _protocol.ScheduleStatus
    target: _protocol.AgentId | None  # optional
    updated_at: str
