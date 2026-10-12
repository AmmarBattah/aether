# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ReplyPaused(TypedDict, total=False):
    """
    Reply to `Pause`: the agents it moved, root first, empty when they
    were already paused; `mid_turn` names those whose current turn must
    finish before the pause takes effect.
    """
    data: dict[str, Any]
    reply: Literal["paused"]
