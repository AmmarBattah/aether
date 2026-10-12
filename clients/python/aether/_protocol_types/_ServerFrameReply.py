# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Daemon to client.
# Tagged by `frame`.
class ServerFrameReply(TypedDict, total=False):
    frame: Literal["reply"]
    id: int
    result: _protocol.Result_of_Reply_or_ErrorReply
