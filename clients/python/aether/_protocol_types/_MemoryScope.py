# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._MemoryScopeAgent import MemoryScopeAgent
from ._MemoryScopeGlobal import MemoryScopeGlobal
from ._MemoryScopeRepository import MemoryScopeRepository
from ._MemoryScopeRepositoryRole import MemoryScopeRepositoryRole
from ._MemoryScopeRole import MemoryScopeRole
from ._MemoryScopeSubtree import MemoryScopeSubtree


MemoryScope = (
    MemoryScopeGlobal |
    MemoryScopeRepository |
    MemoryScopeRole |
    MemoryScopeAgent |
    MemoryScopeRepositoryRole |
    MemoryScopeSubtree
)
