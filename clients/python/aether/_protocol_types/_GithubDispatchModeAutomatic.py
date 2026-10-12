# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class GithubDispatchModeAutomatic(TypedDict, total=False):
    """
    Create an Aether-managed checkout and agent for each issue/PR thread.
    The daemon's current model, approval, and network defaults apply.
    """
    mode: Literal["automatic"]
