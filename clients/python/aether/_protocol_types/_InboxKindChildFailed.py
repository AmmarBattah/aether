# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class InboxKindChildFailed(TypedDict, total=False):
    agent: _protocol.AgentId
    error: str
    kind: Literal["child_failed"]
