# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._RuleScopeAgent import RuleScopeAgent
from ._RuleScopeEverywhere import RuleScopeEverywhere
from ._RuleScopeRun import RuleScopeRun
from ._RuleScopeSubtree import RuleScopeSubtree


RuleScope = RuleScopeEverywhere | RuleScopeAgent | RuleScopeSubtree | RuleScopeRun
