# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Something the run left unfinished.
# Tagged by `open`.
class OpenLoopAgent(TypedDict, total=False):
    id: _protocol.AgentId
    name: str
    open: Literal["agent"]
