# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._AgentStatusBlocked import AgentStatusBlocked
from ._AgentStatusCancelled import AgentStatusCancelled
from ._AgentStatusCompleted import AgentStatusCompleted
from ._AgentStatusCreated import AgentStatusCreated
from ._AgentStatusFailed import AgentStatusFailed
from ._AgentStatusImported import AgentStatusImported
from ._AgentStatusReady import AgentStatusReady
from ._AgentStatusRunning import AgentStatusRunning
from ._AgentStatusWaiting import AgentStatusWaiting


AgentStatus = (
    AgentStatusCreated |
    AgentStatusReady |
    AgentStatusRunning |
    AgentStatusWaiting |
    AgentStatusBlocked |
    AgentStatusCompleted |
    AgentStatusFailed |
    AgentStatusCancelled |
    AgentStatusImported
)
