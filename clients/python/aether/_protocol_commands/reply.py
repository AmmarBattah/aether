# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    AgentId,
    Any,
)

def reply_to_question(*, content: str, question_seq: int, to: AgentId, idempotency_key: str | None = None) -> dict[str, Any]:
    """Operator-only reply to one unanswered, unsuperseded question event.
    A stale question is refused without delivering anything.
    """
    frame: dict[str, Any] = {"cmd": "reply_to_question"}
    frame["content"] = content
    if idempotency_key is not None:
        frame["idempotency_key"] = idempotency_key
    frame["question_seq"] = question_seq
    frame["to"] = to
    return frame
