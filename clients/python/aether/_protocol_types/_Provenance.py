# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Provenance(TypedDict, total=False):
    """
    Where an imported run came from. Absent on a run this daemon ran itself.
    """
    exported_at: str
    source_instance: str
    source_run: _protocol.RunId
