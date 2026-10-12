"""Live CLI delegation check; verifies durable children rather than model claims."""
import argparse
import json
from pathlib import Path
import tempfile
import time

from aether_client import AetherClient, init_git_workspace, isolated_env, start_daemon, stop_daemon, protocol


def check(client, model):
    parent = client.create_agent(model=model, effort="low")["id"]
    client.subscribe()
    results = []
    for round_number in [1, 2]:
        before = len(client.events_seen)
        client.send(parent, f"Delegation test round {round_number}: create exactly three child agents, "
                    f"each using model {model} at low effort, to read README.md and return its first heading. "
                    "Do not edit files. Wait for all three results and summarize them.")
        deadline = time.monotonic() + 240
        while time.monotonic() < deadline:
            frame = client.next_frame(timeout=max(1, deadline - time.monotonic()))
            if frame.get("frame") != "event":
                continue
            event = frame["event"]
            if event["kind"]["type"] == "agent.failed":
                raise AssertionError(event["kind"])
            agents = client.request(protocol.list_agents(query={"parent": parent}))["data"]
            if isinstance(agents, dict):
                agents = agents["agents"]
            root = client.get_agent(parent)
            if (len(agents) == 3 * round_number
                    and all(a["status"]["state"] == "completed" for a in agents)
                    and root["status"]["state"] == "waiting"):
                break
        else:
            raise TimeoutError(f"{model}: tracked delegation did not finish")
        assert all(a["model"] == root["model"] for a in agents)
        assert all(a["config"].get("effort") == "low" for a in agents)
        failures = [e for e in client.events_seen[before:] if e["kind"]["type"] == "tool.failed"]
        assert not failures, failures
        results.append({"round": round_number, "children": len(agents), "failed_tools": 0})
        print(json.dumps({"model": model, **results[-1]}), flush=True)
    return {"model": model, "rounds": results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/aether")
    parser.add_argument("--models", nargs="+", default=["codex/gpt-6.1-sol", "codex/gpt-6-luna", "claude-code/haiku"])
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    with tempfile.TemporaryDirectory(prefix="aether-delegation-") as root:
        env = isolated_env(root)
        workspace = Path(root) / "workspace"
        workspace.mkdir()
        (workspace / "README.md").write_text("# Delegation fixture\n\nRead-only check.\n")
        init_git_workspace(workspace)
        try:
            start_daemon(binary, env, str(workspace))
            with AetherClient(env["AETHER_SOCKET"]) as client:
                results = [check(client, model) for model in args.models]
            Path(args.output).write_text(json.dumps(results, indent=2) + "\n")
        finally:
            stop_daemon(binary, env)


if __name__ == "__main__":
    main()
