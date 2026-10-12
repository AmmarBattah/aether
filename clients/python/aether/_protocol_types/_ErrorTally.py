# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ErrorTally(TypedDict, total=False):
    """
    One normalized error class a provider returned recently, and how often.
    """
    class_: _protocol.ModelErrorClass
    count: int
    last_at: str
