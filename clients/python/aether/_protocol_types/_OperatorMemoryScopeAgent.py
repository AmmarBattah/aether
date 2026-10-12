# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Operator applicability, not authorship. Repository identity is resolved by the runtime.
# Tagged by `scope`.
class OperatorMemoryScopeAgent(TypedDict, total=False):
    id: _protocol.AgentId
    scope: Literal["agent"]
