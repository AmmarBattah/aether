# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class InboxKindTaskFailed(TypedDict, total=False):
    error: str
    kind: Literal["task_failed"]
    structured: Any  # optional
    task: _protocol.TaskId
