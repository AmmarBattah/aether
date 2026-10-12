# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class MemoryProvenance(TypedDict, total=False):
    actor: _protocol.Actor | None  # optional
    agent: _protocol.AgentId | None  # optional
    artifact: _protocol.ArtifactId | None  # optional
    event: _protocol.EventId | None  # optional
    source: _protocol.MemorySource | None  # optional
    task: _protocol.TaskId | None  # optional
