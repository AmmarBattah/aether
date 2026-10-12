# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# The variant of an [`AgentStatus`] without its payload.
AgentStatusKind = Literal["created", "ready", "running", "waiting", "blocked", "completed", "failed", "cancelled", "imported"]
