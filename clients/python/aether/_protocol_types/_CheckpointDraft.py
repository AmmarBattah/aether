# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class CheckpointDraft(TypedDict, total=False):
    """
    The model-facing checkpoint schema. This is what a compactor model is
    asked to produce; the runtime wraps it into a [`Checkpoint`].
    """
    artifacts: list[_protocol.ArtifactNote]  # optional
    constraints: list[str]  # optional
    decisions: list[str]  # optional
    dropped_constraints: list[_protocol.DroppedItem]  # optional
    objective: str
    open_loops: list[str]  # optional
    resolved_open_loops: list[str]  # optional
    state: str
    uncertainty: list[str]  # optional
