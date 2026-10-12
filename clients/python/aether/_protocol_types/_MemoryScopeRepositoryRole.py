# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class MemoryScopeRepositoryRole(TypedDict, total=False):
    """
    A repository's notes for one role family: agents in repository `id`
    whose role is `role` or extends it at a dot (see [`role_families`]).
    """
    id: str
    role: str
    scope: Literal["repository_role"]
