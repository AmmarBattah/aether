# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    TaskId,
)

def publish_artifact(*, content_type: str, name: str, summary: str, task: TaskId, text: str) -> dict[str, Any]:
    """Publish UTF-8 evidence associated with a durable task. The daemon owns
    blob storage and derives the agent/run association from that task.
    """
    frame: dict[str, Any] = {"cmd": "publish_artifact"}
    frame["content_type"] = content_type
    frame["name"] = name
    frame["summary"] = summary
    frame["task"] = task
    frame["text"] = text
    return frame
