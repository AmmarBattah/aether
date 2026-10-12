# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# What an archive command marks: one finished agent, every finished agent of
# a run, or every finished agent there is.
ArchiveTarget = Literal["all_finished"] | dict[str, Any]
