# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Milestone(TypedDict, total=False):
    """
    One line of the run's story: the events a person would want to see.
    """
    agent: _protocol.AgentId | None  # optional
    at: str
    detail: str
    kind: str
