# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ImportSummary(TypedDict, total=False):
    """
    What an import recreated.
    """
    agents: int
    artifact_payloads: int
    artifacts: int
    checkpoints: int
    events: int
    exported_at: str
    invocations: int
    run: _protocol.RunId
    snapshots: int
    source_instance: str
    source_run: _protocol.RunId
    tasks: int
