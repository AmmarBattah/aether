# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Where a remembered decision applies.
# Tagged by `scope`.
class RuleScopeEverywhere(TypedDict, total=False):
    """
    Every agent this daemon runs.
    """
    scope: Literal["everywhere"]
