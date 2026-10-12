# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._ActorAgent import ActorAgent
from ._ActorClient import ActorClient
from ._ActorImported import ActorImported
from ._ActorRule import ActorRule
from ._ActorScheduler import ActorScheduler
from ._ActorSystem import ActorSystem
from ._ActorTool import ActorTool
from ._ActorUser import ActorUser


Actor = (
    ActorSystem |
    ActorScheduler |
    ActorUser |
    ActorAgent |
    ActorTool |
    ActorRule |
    ActorClient |
    ActorImported
)
