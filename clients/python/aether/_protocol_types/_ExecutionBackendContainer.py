# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ExecutionBackendContainer(TypedDict, total=False):
    """
    A container per call, with the workspace mounted writable at the same
    path, everything else in scope mounted read-only, and network per the
    scope's policy.
    """
    backend: Literal["container"]
    image: str
