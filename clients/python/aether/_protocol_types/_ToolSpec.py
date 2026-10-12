# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ToolSpec(TypedDict, total=False):
    """
    Tool definition as exposed to the model.
    """
    description: str
    input_schema: Any
    name: str
    strict: bool  # optional
