# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class RunBody(TypedDict, total=False):
    """
    Everything derived from the run's own work. Carries no local sequence
    numbers, so two stores holding the same run produce equal bodies once
    their ids are put side by side ([`RunReport::canonical_json`]).
    """
    agents: list[_protocol.AgentLine]
    approvals: list[_protocol.ApprovalLine]
    artifacts: list[_protocol.ArtifactLine]
    duration_ms: int
    ended_at: str | None  # optional
    events: int
    failures: list[_protocol.FailureLine]
    objective: str | None  # optional
    open_loops: list[_protocol.OpenLoop]
    outcome: _protocol.RunOutcome
    started_at: str
    tasks: list[_protocol.TaskLine]
    timeline: list[_protocol.Milestone]
    tools: list[_protocol.ToolLine]
    usage: _protocol.Usage
