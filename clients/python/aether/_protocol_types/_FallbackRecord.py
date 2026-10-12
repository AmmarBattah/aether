# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class FallbackRecord(TypedDict, total=False):
    """
    The last time a turn gave up on a provider and moved to the next link of
    its chain.
    """
    at: str
    class_: _protocol.ModelErrorClass | None  # optional
    reason: str
    to: _protocol.ModelRef
