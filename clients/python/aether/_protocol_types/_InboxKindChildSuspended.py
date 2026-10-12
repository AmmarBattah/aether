# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class InboxKindChildSuspended(TypedDict, total=False):
    """
    A descendant parked on provider exhaustion; it probes and resumes by
    itself, so the recipient is informed rather than asked to act.
    """
    agent: _protocol.AgentId
    detail: str
    kind: Literal["child_suspended"]
