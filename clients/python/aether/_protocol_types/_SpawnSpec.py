# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class SpawnSpec(TypedDict, total=False):
    """
    Root agent + task each spawn occurrence creates, in a new run. Fields as on
    `create_agent`; no parent or existing run.
    """
    approval: _protocol.ApprovalPolicy | None  # optional
    config: _protocol.AgentConfig | None  # optional
    fallbacks: list[_protocol.ModelRef]  # optional
    model: _protocol.ModelRef | None  # optional
    name: str | None  # optional
    objective: str
    role: str
    scope: _protocol.Scope | None  # optional
