# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ScheduleOutcomeSpawned(TypedDict, total=False):
    """
    A spawn schedule started a run: its root agent and task.
    """
    agent: _protocol.AgentId
    outcome: Literal["spawned"]
    run: _protocol.RunId
    task: _protocol.TaskId
