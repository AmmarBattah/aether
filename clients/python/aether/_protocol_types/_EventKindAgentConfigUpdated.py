# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindAgentConfigUpdated(TypedDict, total=False):
    """
    An operator or orchestrator changed the agent's configuration mid-run
    (model, effort, tool exposure, approval policy, budget).
    """
    config: _protocol.AgentConfig
    model: _protocol.ModelRef | None  # optional
    name: str | None  # optional
    scope: _protocol.Scope | None  # optional
    type: Literal["agent.config_updated"]
