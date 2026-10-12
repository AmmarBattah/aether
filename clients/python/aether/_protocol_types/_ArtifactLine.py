# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ArtifactLine(TypedDict, total=False):
    agent: _protocol.AgentId | None  # optional
    created_at: str
    evicted: bool  # optional
    id: _protocol.ArtifactId
    kind: str
    name: str
    size_bytes: int
    summary: str
    task: _protocol.TaskId | None  # optional
