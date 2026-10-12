# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# What a tool does to the world. Drives retry policy, idempotency tracking
# and approval requirements.
SideEffectClass = Literal["read_only", "idempotent_mutation", "mutation", "external"]
