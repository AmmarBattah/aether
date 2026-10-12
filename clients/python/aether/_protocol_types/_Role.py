# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict


# Opaque role label; an orchestration layer gives it meaning.
# The harness reads only its dots, to pick a repository's notes for a role
# family (`memory::role_families`).
Role = str
