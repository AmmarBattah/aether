# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class BlockReasonResource(TypedDict, total=False):
    """
    VISION §3.6 limit 5: a live turn waiting for a resource key another
    agent holds. The turn itself resumes when the key comes free.
    """
    block: Literal["resource"]
    holder: _protocol.AgentId
    key: _protocol.ResourceKey
