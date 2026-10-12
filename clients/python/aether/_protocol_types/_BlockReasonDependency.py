# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Why an agent is Blocked. One variant per cause, each carrying what that
# cause needs, so a Blocked agent is never ambiguous about what would clear
# it. An operator pause is not here: it is orthogonal to scheduling state
# and lives in [`Agent::paused`] (VISION §3.1).
# Tagged by `block`.
class BlockReasonDependency(TypedDict, total=False):
    """
    VISION §3.2: the owner of a task whose dependencies are not terminal.
    Cleared only when that dependency finishes.
    """
    block: Literal["dependency"]
    on: _protocol.TaskId
