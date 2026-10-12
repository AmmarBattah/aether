# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class CompactionTriggerHandoff(TypedDict, total=False):
    """
    The agent's model moved across the engine/API boundary. `from` is the
    model that produced the context being folded, so it can compact it.
    """
    from_: _protocol.ModelRef
    session: str | None  # optional
    to: _protocol.ModelRef
    trigger: Literal["handoff"]
