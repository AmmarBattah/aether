# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class AuthInfo(TypedDict, total=False):
    configured: bool
    detail: str | None  # optional
    methods: list[str]  # optional
    program: str | None  # optional
    provider: str
    provider_name: str
    source: str | None  # optional
    subscription: bool  # optional
