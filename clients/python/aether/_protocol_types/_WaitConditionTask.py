# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# What a waiting agent is waiting for. A typed expression the scheduler
# evaluates; the model never polls. Leaves are single facts, and `Any`,
# `All` and `AtLeast` combine them, so "wait for two of these five workers
# or a deadline" is a value, not a prompt.
# Tagged by `on`.
class WaitConditionTask(TypedDict, total=False):
    """
    The task reached a terminal state.
    """
    id: _protocol.TaskId
    on: Literal["task"]
