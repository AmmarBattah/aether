# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Who caused an event.
# Tagged by `actor`.
class ActorSystem(TypedDict, total=False):
    actor: Literal["system"]
