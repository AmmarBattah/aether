# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class LowDisk(TypedDict, total=False):
    """
    The filesystem holding the daemon's data is below its floor: new turns
    wait until it has room again. Turns already running carry on.
    """
    available_bytes: int
    floor_bytes: int
    path: str
    since: str
