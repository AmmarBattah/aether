# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ArtifactNote(TypedDict, total=False):
    """
    A reference to an important artifact, kept as a note rather than a copy.
    """
    note: str
    reference: str | None  # optional
