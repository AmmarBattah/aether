# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ProviderUsage(TypedDict, total=False):
    """
    Latest normalized account report. The observation time is not a reset time.
    """
    observed_at: str
    windows: list[_protocol.UsageWindow]
