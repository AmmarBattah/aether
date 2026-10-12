# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Checkpoint(TypedDict, total=False):
    """
    A durable, validated checkpoint.
    """
    agent_id: _protocol.AgentId
    created_at: str
    draft: _protocol.CheckpointDraft
    id: _protocol.CheckpointId
    provenance: _protocol.CheckpointProvenance
    version: int
