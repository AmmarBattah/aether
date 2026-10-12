# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class AgentQuery(TypedDict, total=False):
    include_archived: bool  # optional
    live_only: bool  # optional
    parent: _protocol.AgentId | None  # optional
    run: _protocol.RunId | None  # optional
    statuses: list[_protocol.AgentStatusKind]  # optional
