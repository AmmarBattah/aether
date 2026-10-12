# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class BlockReasonExhausted(TypedDict, total=False):
    """
    Provider credits/allowance spent on every link of the chain. Cleared by
    the scheduler when `next_probe` comes due, or by a person.
    """
    block: Literal["exhausted"]
    class_: _protocol.ExhaustionClass
    detail: str
    model: _protocol.ModelRef
    next_probe: str
    probes: int
    reported_reset: str | None  # optional
    since: str
