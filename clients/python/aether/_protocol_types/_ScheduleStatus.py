# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._ScheduleStatusActive import ScheduleStatusActive
from ._ScheduleStatusCancelled import ScheduleStatusCancelled
from ._ScheduleStatusExhausted import ScheduleStatusExhausted
from ._ScheduleStatusFailed import ScheduleStatusFailed
from ._ScheduleStatusPaused import ScheduleStatusPaused


ScheduleStatus = (
    ScheduleStatusActive |
    ScheduleStatusPaused |
    ScheduleStatusCancelled |
    ScheduleStatusExhausted |
    ScheduleStatusFailed
)
