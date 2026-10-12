# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


# How the model should think. Adapters honor what the capability profile
# supports and report the rest as unsupported.
# Tagged by `mode`.
class ThinkingModeOff(TypedDict, total=False):
    mode: Literal["off"]
