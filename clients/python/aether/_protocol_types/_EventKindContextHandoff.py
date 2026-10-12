# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindContextHandoff(TypedDict, total=False):
    """
    An agent's context crossed the engine/API boundary. Names what carried
    over and, honestly, what did not.
    """
    carried: int
    checkpoint: _protocol.CheckpointId | None  # optional
    from_: _protocol.ModelRef
    lost: list[str]  # optional
    to: _protocol.ModelRef
    type: Literal["context.handoff"]
