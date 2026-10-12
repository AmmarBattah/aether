# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindAgentArchived(TypedDict, total=False):
    """
    The agent was cleared out of the live view, or brought back.
    A view mark: nothing is removed, and replay is unaffected.
    """
    archived: bool
    type: Literal["agent.archived"]
