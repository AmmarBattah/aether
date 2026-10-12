# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._EndedCancelled import EndedCancelled
from ._EndedCompleted import EndedCompleted
from ._EndedFailed import EndedFailed
from ._EndedRunning import EndedRunning


Ended = EndedRunning | EndedCompleted | EndedFailed | EndedCancelled
