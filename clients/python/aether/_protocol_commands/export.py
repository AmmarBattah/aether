# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    RunId,
)

def export_run(*, path: str, run: RunId) -> dict[str, Any]:
    """Write a finished run's events, snapshots, checkpoints and artifacts
    to one file on the daemon's machine.
    """
    frame: dict[str, Any] = {"cmd": "export_run"}
    frame["path"] = path
    frame["run"] = run
    return frame
