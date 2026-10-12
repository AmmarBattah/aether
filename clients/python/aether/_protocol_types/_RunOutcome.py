# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# A run's state, read from its agents rather than declared anywhere.
RunOutcome = Literal["completed", "failed", "cancelled", "running"]
