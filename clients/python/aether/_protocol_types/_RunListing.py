# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class RunListing(TypedDict, total=False):
    """
    One row of `aether runs list`, projected from a run's agents and tasks.
    """
    agents: int
    agents_imported: int  # optional
    agents_live: int
    objective: str | None  # optional
    run: _protocol.RunId
    started_at: str
    updated_at: str
    usage: _protocol.Usage
