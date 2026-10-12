# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# What approval policy an agent gets when nothing else decides one. The
# shipped default is `Yolo`: the operator asked for auto-approve out of the
# box, and the execution backend - not the prompt - is the isolation
# boundary.
ApprovalDefault = Literal["yolo", "ask"]
