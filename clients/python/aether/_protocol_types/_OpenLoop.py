# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._OpenLoopAgent import OpenLoopAgent
from ._OpenLoopApproval import OpenLoopApproval
from ._OpenLoopTask import OpenLoopTask


OpenLoop = OpenLoopAgent | OpenLoopTask | OpenLoopApproval
