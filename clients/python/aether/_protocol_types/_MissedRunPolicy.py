# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# When several occurrences elapsed between scheduler ticks, coalescing
# delivers one trigger; skipping advances the clock without delivering one.
# A tick that is late by less than one recurrence still delivers normally.
MissedRunPolicy = Literal["coalesce", "skip"]
