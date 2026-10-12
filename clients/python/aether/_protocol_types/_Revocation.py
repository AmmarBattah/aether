# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Revocation(TypedDict, total=False):
    """
    Why a rule stopped applying. Kept on the record so the log explains a
    decision that would be made differently today.
    """
    at: str
    by: _protocol.Actor
    reason: str | None  # optional
