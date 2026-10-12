# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# Server-observed connection identity, never a bearer token or client-supplied name.
# Tagged by `transport`.
class OperatorOriginLocalSocket(TypedDict, total=False):
    connection: str
    transport: Literal["local_socket"]
