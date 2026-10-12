# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._ScheduleOutcomeDelivered import ScheduleOutcomeDelivered
from ._ScheduleOutcomeFailed import ScheduleOutcomeFailed
from ._ScheduleOutcomeSkipped import ScheduleOutcomeSkipped
from ._ScheduleOutcomeSpawned import ScheduleOutcomeSpawned


ScheduleOutcome = (
    ScheduleOutcomeDelivered |
    ScheduleOutcomeSpawned |
    ScheduleOutcomeSkipped |
    ScheduleOutcomeFailed
)
