# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class SeedReport(TypedDict, total=False):
    """
    What one pass of reading a repository's committed notes did.
    """
    admitted: int
    found: int
    rejected: int
    retracted: int  # optional
    superseded: int
    unchanged: int
