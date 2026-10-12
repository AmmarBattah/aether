# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class EventKindMcpServerFailed(TypedDict, total=False):
    """
    A configured MCP server could not be reached or stopped answering. Its
    tools are withdrawn; a call against it fails rather than hanging.
    """
    error: str
    server: str
    type: Literal["mcp.server_failed"]
