# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
)

def sync_repository_knowledge(*, root: str | None = None) -> dict[str, Any]:
    """Read a repository's committed notes into memory scoped to it.

    The scope is derived from the root, never named by the caller: reach
    to the socket is the whole authorization boundary (ARCHITECTURE §16),
    and a caller that could name both would be able to read notes out of a
    directory it just wrote and admit them into any repository's memory,
    attributed to the person.
    """
    frame: dict[str, Any] = {"cmd": "sync_repository_knowledge"}
    if root is not None:
        frame["root"] = root
    return frame
