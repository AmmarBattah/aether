# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class MemoryCandidate(TypedDict, total=False):
    """
    A proposed memory. Admission decides whether it becomes a record.
    """
    confidence: float  # optional
    importance: float  # optional
    kind: _protocol.MemoryKind
    provenance: _protocol.MemoryProvenance  # optional
    scope: _protocol.MemoryScope
    structured: Any  # optional
    supersedes: _protocol.MemoryId | None  # optional
    tags: list[str]  # optional
    text: str
