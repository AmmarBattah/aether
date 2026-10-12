"""Shared offline benchmark-test fixtures."""
from pathlib import Path

import harness_bench as hb

TEST_SCRATCH = Path(hb.REPO_ROOT) / "target/hb-tests"
TEST_SCRATCH.mkdir(parents=True, exist_ok=True)


def node(agent_id, state, children=(), model=None, effort=None, role="coder"):
    return {"agent": {"id": agent_id, "role": role, "model": model or {"provider": "claude-code", "model": "sonnet"},
                       "config": {"effort": effort}, "status": ({"state": state, "condition": {"on": "inbox"}} if state == "waiting" else {"state": state})},
            "children": list(children)}
