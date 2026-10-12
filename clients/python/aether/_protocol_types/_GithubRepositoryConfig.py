# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class GithubRepositoryConfig(TypedDict, total=False):
    agent: _protocol.AgentId | None  # optional
    allowed_users: list[str] | None  # optional
    dispatch: _protocol.GithubDispatchMode  # optional
    enabled: bool
    repository: str
