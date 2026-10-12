# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Where an agent's tools actually run.
# Tagged by `backend`.
class ExecutionBackendHost(TypedDict, total=False):
    """
    The daemon's own machine, inside the scoped roots.
    """
    backend: Literal["host"]
