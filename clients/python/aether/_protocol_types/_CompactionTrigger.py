# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._CompactionTriggerExplicit import CompactionTriggerExplicit
from ._CompactionTriggerHandoff import CompactionTriggerHandoff
from ._CompactionTriggerLargeToolOutput import CompactionTriggerLargeToolOutput
from ._CompactionTriggerLongWait import CompactionTriggerLongWait
from ._CompactionTriggerTaskBoundary import CompactionTriggerTaskBoundary
from ._CompactionTriggerTokenThreshold import CompactionTriggerTokenThreshold


CompactionTrigger = (
    CompactionTriggerTokenThreshold |
    CompactionTriggerTaskBoundary |
    CompactionTriggerLargeToolOutput |
    CompactionTriggerLongWait |
    CompactionTriggerExplicit |
    CompactionTriggerHandoff
)
