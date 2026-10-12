# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class TaskResult(TypedDict, total=False):
    """
    The durable outcome of a task. Cheap to carry in context forever.
    """
    artifacts: list[_protocol.ArtifactId]  # optional
    structured: Any  # optional
    summary: str
