# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._BlockReasonDependency import BlockReasonDependency
from ._BlockReasonExhausted import BlockReasonExhausted
from ._BlockReasonInspection import BlockReasonInspection
from ._BlockReasonResource import BlockReasonResource


BlockReason = (
    BlockReasonDependency |
    BlockReasonResource |
    BlockReasonInspection |
    BlockReasonExhausted
)
