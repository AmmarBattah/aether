# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ToolResultSucceeded(TypedDict, total=False):
    artifacts: list[_protocol.ArtifactId]  # optional
    content: str
    duration_ms: int
    is_error: bool  # optional
    result: Literal["succeeded"]
    truncated: bool  # optional
