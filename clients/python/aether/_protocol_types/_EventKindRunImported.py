# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindRunImported(TypedDict, total=False):
    """
    A run was replayed into this store from another daemon's export. It is
    the imported run's first event, so everything after it reads as
    history that happened elsewhere.
    """
    events: int
    exported_at: str
    source_instance: str
    source_run: _protocol.RunId
    type: Literal["run.imported"]
