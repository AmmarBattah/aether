# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._TaskStatusActive import TaskStatusActive
from ._TaskStatusAssigned import TaskStatusAssigned
from ._TaskStatusBlocked import TaskStatusBlocked
from ._TaskStatusCancelled import TaskStatusCancelled
from ._TaskStatusCompleted import TaskStatusCompleted
from ._TaskStatusFailed import TaskStatusFailed
from ._TaskStatusQueued import TaskStatusQueued


TaskStatus = (
    TaskStatusQueued |
    TaskStatusAssigned |
    TaskStatusActive |
    TaskStatusBlocked |
    TaskStatusCompleted |
    TaskStatusFailed |
    TaskStatusCancelled
)
