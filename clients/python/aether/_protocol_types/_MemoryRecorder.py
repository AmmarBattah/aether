# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# Who recorded a memory, as a query names it: read from the runtime-set
# `provenance.actor`, never from anything an author supplied.
MemoryRecorder = Literal["operator"]
