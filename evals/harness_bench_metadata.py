"""Allowlisted configuration and resolved native-session evidence for benchmarks.

Configuration is not proof of the effective model or effort. Capture actual
session metadata separately, including sessions launched through Aether.
"""
import json
import os
from pathlib import Path
import tomllib


def native_configured_defaults(name):
    """Configuration evidence is not a claim about effective runtime defaults."""
    if name == "codex":
        path = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex"))) / "config.toml"
    else:
        path = Path(os.environ.get("CLAUDE_CONFIG_DIR", str(Path.home() / ".claude"))) / "settings.json"
    if not path.is_file():
        return {"source": str(path), "available": False}
    try:
        config = tomllib.loads(path.read_text()) if name == "codex" else json.loads(path.read_text())
        selected = {key: config[key] for key in ("model", "model_reasoning_effort", "effortLevel", "profile")
                    if key in config}
        if name == "codex" and config.get("profile"):
            profile = config.get("profiles", {}).get(config["profile"], {})
            selected.update({key: profile[key] for key in ("model", "model_reasoning_effort") if key in profile})
        return {"source": str(path), "available": True, **selected}
    except (OSError, ValueError) as error:
        return {"source": str(path), "error": f"{type(error).__name__}: {error}"}


def codex_session_metadata(thread_ids, env):
    """Read only the named sessions; never export prompts or config credentials."""
    metadata = []
    home = Path(env.get("CODEX_HOME", str(Path.home() / ".codex")))
    valid_ids = {value for value in thread_ids if isinstance(value, str) and value
                 and all(c in "0123456789abcdef-" for c in value)}
    for thread_id in sorted(valid_ids):
        for path in sorted((home / "sessions").glob(f"**/*{thread_id}.jsonl")):
            with path.open() as stream:
                for line in stream:
                    try:
                        event = json.loads(line)
                    except ValueError:
                        continue
                    if not isinstance(event, dict) or event.get("type") != "turn_context":
                        continue
                    payload = event.get("payload")
                    if not isinstance(payload, dict):
                        continue
                    item = {"source": "native rollout turn_context", "session_id": thread_id,
                            "model": payload.get("model"), "effort": payload.get("effort")}
                    if item not in metadata:
                        metadata.append(item)
    return metadata


def native_resolved_metadata(stdout_text, env, is_claude):
    metadata = []
    thread_ids = []
    for line in stdout_text.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        if is_claude and event.get("type") == "system" and event.get("subtype") == "init":
            metadata.append({"source": "native system.init", "model": event.get("model"),
                             "effort": event.get("effort"), "session_id": event.get("session_id")})
        if not is_claude and event.get("type") == "thread.started":
            thread_ids.append(event.get("thread_id"))
    return metadata + codex_session_metadata(thread_ids, env)


def aether_resolved_metadata(events, agents, env):
    providers = {agent["id"]: agent["model"]["provider"] for agent in agents}
    metadata = []
    for event in events:
        kind = event.get("kind", {})
        if kind.get("type") != "custom" or kind.get("name") != "engine.session":
            continue
        agent_id = event.get("agent_id")
        payload = kind.get("payload", {})
        session = payload.get("session")
        # A requested default/effort is not an effective one. Claude currently
        # exposes the resolved model here but no resolved effort.
        metadata.append({"source": "aether engine.session", "agent_id": agent_id,
                         "session_id": session, "model": payload.get("model"), "effort": None})
        if providers.get(agent_id) == "codex":
            metadata.extend({**item, "agent_id": agent_id}
                            for item in codex_session_metadata([session], env))
    return metadata
