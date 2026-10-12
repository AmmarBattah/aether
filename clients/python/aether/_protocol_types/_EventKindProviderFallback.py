# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindProviderFallback(TypedDict, total=False):
    """
    A subscription engine reported how much of its window is spent.
    A turn moved to the next link of its model chain (VISION §4.5).
    """
    class_: _protocol.ModelErrorClass | None  # optional
    from_: _protocol.ModelRef
    reason: str
    to: _protocol.ModelRef
    type: Literal["provider.fallback"]
