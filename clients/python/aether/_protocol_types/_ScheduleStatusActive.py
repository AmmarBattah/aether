# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Tagged by `state`.
class ScheduleStatusActive(TypedDict, total=False):
    next_due: str
    state: Literal["active"]
