# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ExportSummary(TypedDict, total=False):
    """
    What an export wrote.
    """
    agents: int
    artifact_payloads: int
    artifacts: int
    bytes_written: int
    checkpoints: int
    events: int
    invocations: int
    path: str
    run: _protocol.RunId
    snapshots: int
    tasks: int
