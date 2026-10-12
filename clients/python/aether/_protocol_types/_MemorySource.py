# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class MemorySource(TypedDict, total=False):
    """
    A committed file a memory was read from, and the commit it was read at.
    The commit is absent when the tree is not a git repository, or git is not
    on the machine: the note still stands, its version is just unknown.
    """
    commit: str | None  # optional
    file: str
