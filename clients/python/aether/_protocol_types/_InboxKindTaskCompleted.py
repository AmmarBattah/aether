# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Tagged by `kind`.
class InboxKindTaskCompleted(TypedDict, total=False):
    kind: Literal["task_completed"]
    result: _protocol.TaskResult
    task: _protocol.TaskId
