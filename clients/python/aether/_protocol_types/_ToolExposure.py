# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._ToolExposureAll import ToolExposureAll
from ._ToolExposureExcept import ToolExposureExcept
from ._ToolExposureOnly import ToolExposureOnly


ToolExposure = ToolExposureAll | ToolExposureOnly | ToolExposureExcept
