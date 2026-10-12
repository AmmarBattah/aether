# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ArtifactQuery(TypedDict, total=False):
    agent: _protocol.AgentId | None  # optional
    limit: int  # optional
    name: str | None  # optional
    task: _protocol.TaskId | None  # optional
    with_bytes_only: bool  # optional
