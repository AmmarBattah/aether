# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# A change to an agent's wall-clock stop (§3.1). Every knob has to be
# changeable on a live agent, and one that can only ever move further into
# the future is not: removing the deadline needs a form of its own.
DeadlineUpdate = Literal["clear"] | dict[str, Any]
