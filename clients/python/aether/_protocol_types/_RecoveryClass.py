# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# How an interrupted turn should be handled after a restart.
RecoveryClass = Literal["resume", "retry", "inspect"]
