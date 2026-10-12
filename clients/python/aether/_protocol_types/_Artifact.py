# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Artifact(TypedDict, total=False):
    agent_id: _protocol.AgentId | None  # optional
    content_type: str | None  # optional
    created_at: str
    created_by: _protocol.Actor
    digest: str | None  # optional
    evicted_at: str | None  # optional
    id: _protocol.ArtifactId
    kind: _protocol.ArtifactKind
    name: str
    path: str | None  # optional
    size_bytes: int
    summary: str
    task_id: _protocol.TaskId | None  # optional
