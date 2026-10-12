# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class ReplyRunImported(TypedDict, total=False):
    data: _protocol.ImportSummary
    reply: Literal["run_imported"]
