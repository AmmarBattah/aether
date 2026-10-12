# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindMcpServerConnected(TypedDict, total=False):
    """
    A configured MCP server answered and its tools are offered to agents.
    """
    server: str
    tools: list[str]
    type: Literal["mcp.server_connected"]
