# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._WaitConditionAll import WaitConditionAll
from ._WaitConditionAny import WaitConditionAny
from ._WaitConditionApproval import WaitConditionApproval
from ._WaitConditionAtLeast import WaitConditionAtLeast
from ._WaitConditionDeadline import WaitConditionDeadline
from ._WaitConditionInbox import WaitConditionInbox
from ._WaitConditionMessageFrom import WaitConditionMessageFrom
from ._WaitConditionTask import WaitConditionTask


WaitCondition = (
    WaitConditionTask |
    WaitConditionApproval |
    WaitConditionMessageFrom |
    WaitConditionInbox |
    WaitConditionDeadline |
    WaitConditionAny |
    WaitConditionAll |
    WaitConditionAtLeast
)
