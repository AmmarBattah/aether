# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class GithubRepositoryStatus(TypedDict, total=False):
    dispatched: int
    ignored: int
    last_error: str | None  # optional
    last_poll_at: str | None  # optional
    last_success_at: str | None  # optional
    next_poll_at: str | None  # optional
    repository: str
