# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._ExecutionBackendContainer import ExecutionBackendContainer
from ._ExecutionBackendHost import ExecutionBackendHost


ExecutionBackend = ExecutionBackendHost | ExecutionBackendContainer
