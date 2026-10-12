# Generated from clients/protocol.schema.json by scripts/generate_clients.py.
# Do not edit: change the Rust types and regenerate.
from __future__ import annotations

from ..protocol import (
    Any,
    GithubWatcherConfig,
)

def configure_github_watcher(*, config: GithubWatcherConfig, expected_revision: int) -> dict[str, Any]:
    """Replace watcher settings only if no other operator changed them."""
    frame: dict[str, Any] = {"cmd": "configure_github_watcher"}
    frame["config"] = config
    frame["expected_revision"] = expected_revision
    return frame
