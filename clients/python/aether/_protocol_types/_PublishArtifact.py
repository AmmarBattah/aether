# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class PublishArtifact(TypedDict, total=False):
    """
    Publish UTF-8 evidence associated with a durable task. The daemon owns
    blob storage and derives the agent/run association from that task.
    """
    content_type: str
    name: str
    summary: str
    task: _protocol.TaskId
    text: str
