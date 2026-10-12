# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class FailureLine(TypedDict, total=False):
    """
    What failed, and what it was. Every failure in the run, in order.
    """
    agent: _protocol.AgentId | None  # optional
    at: str
    error: str
    kind: str
