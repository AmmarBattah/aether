# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
)

def import_run(*, path: str) -> dict[str, Any]:
    """Recreate a run from such a file, under fresh ids."""
    frame: dict[str, Any] = {"cmd": "import_run"}
    frame["path"] = path
    return frame
