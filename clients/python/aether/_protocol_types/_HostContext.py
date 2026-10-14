# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# Engine host setup a session loads. Lean drops Claude Code's native skills
# and non-Aether MCP servers; other engines ignore it.
HostContext = Literal["full", "lean"]
