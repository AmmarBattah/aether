# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class GithubWatcherConfig(TypedDict, total=False):
    enabled: bool  # optional
    label_regex: str  # optional
    mention_regex: str  # optional
    operator_users: list[str] | None  # optional
    poll_interval_secs: int  # optional
    repositories: list[_protocol.GithubRepositoryConfig]  # optional
