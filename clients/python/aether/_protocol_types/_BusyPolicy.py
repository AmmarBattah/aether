# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# What an occurrence does when the target is busy. Either way at most one
# trigger from a schedule is ever pending in the target's inbox.
BusyPolicy = Literal["skip", "queue_one"]
