# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# Normalized failure class. Every adapter maps its provider's errors here.
ModelErrorClass = Literal["rate_limit", "overloaded", "context_limit", "invalid_request", "not_found", "network", "timeout", "cancelled", "server", "unknown", "auth", "permission", "quota_exhausted", "usage_limit"]
