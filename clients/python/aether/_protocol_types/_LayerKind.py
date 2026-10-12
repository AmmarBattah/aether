# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# The ordered context layers. Order is the order they are rendered.
LayerKind = Literal["harness_policy", "model_profile", "role_task", "runtime_state", "checkpoint", "retrieved_memory", "recent_tail", "tool_schemas"]
