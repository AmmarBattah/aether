# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class AgentStatusImported(TypedDict, total=False):
    """
    Read out of another daemon's export. Terminal by construction: an
    imported agent is a record of work that happened elsewhere, and no
    local command or scheduler pass may give it a turn.
    """
    state: Literal["imported"]
    was: _protocol.AgentStatusKind
