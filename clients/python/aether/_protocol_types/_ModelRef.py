# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ModelRef(TypedDict, total=False):
    """
    A provider-qualified model name, rendered as `provider/model`.
    """
    model: str
    provider: str
