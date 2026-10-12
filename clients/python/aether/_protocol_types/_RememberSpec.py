# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class RememberSpec(TypedDict, total=False):
    """
    How to remember the answer a person just gave. The defaults are the
    narrowest reading of it: the answered agent, this tool, and what this
    call touched. Anything wider is asked for explicitly.
    """
    note: str | None  # optional
    scope: _protocol.RuleScope | None  # optional
    tool_wide: bool  # optional
