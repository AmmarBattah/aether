# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindCompactionFailed(TypedDict, total=False):
    """
    `models` names what was asked to fold, in the order it was tried: a
    transcript that reached a provider that then failed leaves no other
    durable trace of the egress.
    """
    error: str
    models: list[_protocol.ModelRef]  # optional
    type: Literal["compaction.failed"]
