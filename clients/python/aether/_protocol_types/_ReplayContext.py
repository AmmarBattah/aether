# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ReplayContext(TypedDict, total=False):
    """
    The shape of what the model was given. The layer texts are here; the
    whole snapshot is a separate record, named so a caller can fetch it.
    """
    layers: list[_protocol.ContextLayer]
    model: _protocol.ModelRef
    snapshot: _protocol.SnapshotId
    token_estimate: int
