# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._InboxKindApprovalResolved import InboxKindApprovalResolved
from ._InboxKindChildFailed import InboxKindChildFailed
from ._InboxKindChildResumed import InboxKindChildResumed
from ._InboxKindChildSuspended import InboxKindChildSuspended
from ._InboxKindExternalTrigger import InboxKindExternalTrigger
from ._InboxKindMessage import InboxKindMessage
from ._InboxKindTaskCancelled import InboxKindTaskCancelled
from ._InboxKindTaskCompleted import InboxKindTaskCompleted
from ._InboxKindTaskFailed import InboxKindTaskFailed
from ._InboxKindWake import InboxKindWake


InboxKind = (
    InboxKindTaskCompleted |
    InboxKindTaskFailed |
    InboxKindTaskCancelled |
    InboxKindMessage |
    InboxKindApprovalResolved |
    InboxKindChildFailed |
    InboxKindChildSuspended |
    InboxKindChildResumed |
    InboxKindExternalTrigger |
    InboxKindWake
)
