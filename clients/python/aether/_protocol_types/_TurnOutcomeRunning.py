# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# How the turn ended.
# Tagged by `outcome`.
class TurnOutcomeRunning(TypedDict, total=False):
    """
    The log ends inside this turn: it is the turn in flight.
    """
    outcome: Literal["running"]
