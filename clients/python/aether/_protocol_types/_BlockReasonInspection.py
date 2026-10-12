# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class BlockReasonInspection(TypedDict, total=False):
    """
    VISION §3.9: a non-idempotent tool call started without an outcome.
    Only a person clears it.
    """
    block: Literal["inspection"]
    detail: str
