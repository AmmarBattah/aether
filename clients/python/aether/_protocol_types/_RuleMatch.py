# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class RuleMatch(TypedDict, total=False):
    """
    What a rule matches. An absent field matches anything, so the empty
    matcher covers every call — deliberately, so "allow everything for this
    run" is expressible without a magic pattern.
    """
    argument: str | None  # optional
    resource_prefix: str | None  # optional
    tool: str | None  # optional
