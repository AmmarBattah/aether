# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class Scope(TypedDict, total=False):
    """
    Execution scope: where an agent may act. A first-class runtime attribute,
    not prose in a prompt.
    """
    allowed_paths: list[str]  # optional
    backend: _protocol.ExecutionBackend  # optional
    labels: dict[str, str]  # optional
    network: _protocol.NetworkPolicy  # optional
    repository: str | None  # optional
    workspace_root: str | None  # optional
