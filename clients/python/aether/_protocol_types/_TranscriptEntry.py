# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class TranscriptEntry(TypedDict, total=False):
    at: str
    invocation: _protocol.InvocationId | None  # optional
    message: _protocol.Message
    seq: int
