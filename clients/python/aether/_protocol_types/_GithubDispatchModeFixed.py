# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# How a repository's accepted requests reach an agent.
# Tagged by `mode`.
class GithubDispatchModeFixed(TypedDict, total=False):
    """
    Deliver every trigger to the fixed, operator-chosen `agent`. Stale if
    that agent pauses or goes terminal; an operator must repoint it.
    """
    mode: Literal["fixed"]
