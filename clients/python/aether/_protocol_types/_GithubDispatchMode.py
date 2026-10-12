# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from typing import Any, Literal, TypedDict
from ._GithubDispatchModeAutomatic import GithubDispatchModeAutomatic
from ._GithubDispatchModeFixed import GithubDispatchModeFixed
from ._GithubDispatchModeThread import GithubDispatchModeThread


GithubDispatchMode = (
    GithubDispatchModeFixed |
    GithubDispatchModeAutomatic |
    GithubDispatchModeThread
)
