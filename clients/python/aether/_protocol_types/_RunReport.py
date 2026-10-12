# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from .. import protocol as _protocol


class RunReport(TypedDict, total=False):
    """
    A run's report: a header that says which run and when, and a body that
    says what happened. Only the body is comparable across stores - the
    header carries the local identity.
    """
    body: _protocol.RunBody
    from_seq: int
    generated_at: str
    provenance: _protocol.Provenance | None  # optional
    run: _protocol.RunId
    schema: int
    to_seq: int
