# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# A resource a call touches, used to decide which calls may run together.
# Conventional forms: `file:<absolute path>`, `dir:<absolute path>`,
# `repo:<root>`, `network`, `agent:<id>`.
ResourceKey = str
