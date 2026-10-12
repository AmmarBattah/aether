# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class McpToolInfo(TypedDict, total=False):
    """
    One tool a server offers, as Aether will expose it.
    """
    declared: bool
    name: str
    qualified_name: str
    side_effect: _protocol.SideEffectClass
