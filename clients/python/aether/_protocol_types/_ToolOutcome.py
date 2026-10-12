# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ToolOutcome(TypedDict, total=False):
    """
    What a tool produced.
    """
    artifacts: list[_protocol.ArtifactId]  # optional
    content: str
    duration_ms: int  # optional
    is_error: bool  # optional
    side_effect_occurred: bool | None  # optional
    structured: Any  # optional
    truncated: bool  # optional
