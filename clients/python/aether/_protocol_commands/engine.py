# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def engine_tool(*, agent: AgentId, input: Any, name: str, token: str) -> dict[str, Any]:
    """A harness tool call or permission prompt from an external engine's
    MCP bridge, authenticated by the per-turn token.
    """
    frame: dict[str, Any] = {"cmd": "engine_tool"}
    frame["agent"] = agent
    frame["input"] = input
    frame["name"] = name
    frame["token"] = token
    return frame

def engine_tools(*, agent: AgentId, token: str) -> dict[str, Any]:
    """The tools an engine's MCP bridge should advertise for this agent:
    Aether's harness tools plus the MCP servers' tools it may use.
    """
    frame: dict[str, Any] = {"cmd": "engine_tools"}
    frame["agent"] = agent
    frame["token"] = token
    return frame
