# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
)

def mcp_servers(*, refresh: bool | None = None, server: str | None = None) -> dict[str, Any]:
    """Configured MCP servers and their tools. `refresh` reconnects first,
    so tools added since daemon start are discovered on demand.
    """
    frame: dict[str, Any] = {"cmd": "mcp_servers"}
    if refresh is not None:
        frame["refresh"] = refresh
    if server is not None:
        frame["server"] = server
    return frame
