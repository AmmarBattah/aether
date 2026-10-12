# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# Where a layer's text reached the model on this turn.
LayerPlacement = Literal["system", "user_turn", "retained"]
