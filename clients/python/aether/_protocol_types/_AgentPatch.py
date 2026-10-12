# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class AgentPatch(TypedDict, total=False):
    """
    Partial update to a live agent. Only set fields change.
    """
    approval: _protocol.ApprovalPolicy | None  # optional
    backend: _protocol.ExecutionBackend | None  # optional
    concurrency: int | None  # optional
    cost_budget: float | None  # optional
    deadline: _protocol.DeadlineUpdate | None  # optional
    effort: _protocol.Effort | None  # optional
    fallbacks: list[_protocol.ModelRef] | None  # optional
    instructions: str | None  # optional
    long_lived: bool | None  # optional
    max_turns: int | None  # optional
    model: _protocol.ModelRef | None  # optional
    name: str | None  # optional
    network: _protocol.NetworkPolicy | None  # optional
    priority: _protocol.Priority | None  # optional
    token_budget: int | None  # optional
    tools: _protocol.ToolExposure | None  # optional
