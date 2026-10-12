# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# Where dollar figures came from; independent of whether every invocation was priced.
CostBasis = Literal["list_price", "provider_reported", "mixed", "unknown"]
