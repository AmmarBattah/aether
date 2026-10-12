#!/usr/bin/env python3
"""Measure the authored model profile (VISION §4.4) against its own absence.

Runs the same tasks on the same model under two conditions and compares
outcome, turns, tool calls, tokens, cost and wall time:
  profile  `[model] profiles = true`, forcing the layer on for every family
  plain    `[model] profiles = false` in config.toml, suppressing the layer

Both conditions are written explicitly, so the measurement does not move when
a family's shipped default changes. The knob is
`crates/aether-model/src/config.rs`'s `ModelSection::profiles` (see
`crates/aether-runtime/tests/kernel.rs::profiles_can_be_switched_off`).
A family defaults on only once this eval shows its profile helps or is
neutral; ARCHITECTURE §8.1 records which have.

Usage:
  evals/.venv/bin/python evals/model_profiles.py --model claude-code/haiku --runs 3
  evals/.venv/bin/python evals/model_profiles.py --model codex/gpt-6-luna
  evals/.venv/bin/python evals/model_profiles.py --model mock-gpt/gpt-5-mock

`--model` is repeatable and/or comma-separated; each model is run through
both conditions independently. Results land in `evals/results/`.

An engine model (`claude-code/...`, `codex/...`) goes through
`ModelProfile::text(true)`, which drops the profile's tool half by design.
The tool half only ever goes out on the API path, so measuring it needs an
API provider: the `mock-*` providers below are that path, one per family,
answering from `scripts/mock_openai.py` under the `openai-responses`
adapter. A mock run is structural - it proves which profile text reaches the
wire and that the whole API path carries it, not that a real model does
better with it.
"""

import argparse
import json
import os
import shlex
import shutil
import socket
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from aether_client import AetherClient, isolated_env, start_daemon, stop_daemon  # noqa: E402

FIXTURE = os.path.join(HERE, "fixtures", "pyproj")
MOCK_SERVER = os.path.join(HERE, "..", "scripts", "mock_openai.py")
RUN_TIMEOUT_SECONDS = 1800.0


# ----- fixture / verdict plumbing (mirrors evals/run_tasks.py) -------------


def fresh_workspace(root):
    ws = os.path.join(root, "workspace")
    shutil.copytree(FIXTURE, ws)
    return ws


def tests_digest(ws):
    import hashlib

    h = hashlib.sha256()
    tests = os.path.join(ws, "tests")
    for name in sorted(os.listdir(tests)):
        path = os.path.join(tests, name)
        # A run leaves __pycache__ behind; only the test sources are frozen.
        if not os.path.isfile(path) or not name.endswith(".py"):
            continue
        with open(path, "rb") as f:
            h.update(name.encode() + f.read())
    return h.hexdigest()


def check(ws, task, before_digest):
    if task.get("tests_frozen") and tests_digest(ws) != before_digest:
        return False, "tests were modified"
    proc = subprocess.run(["bash", "-c", task["check"]], cwd=ws, capture_output=True, text=True, timeout=120,
                          env={**os.environ, "PATH": os.path.dirname(sys.executable) + os.pathsep + os.environ["PATH"]})
    return proc.returncode == 0, (proc.stdout + proc.stderr)[-300:]


# ----- per-run isolation -----------------------------------------------------


def write_model_config(config_dir, condition):
    body = "profiles = true" if condition == "profile" else "profiles = false"
    with open(os.path.join(config_dir, "config.toml"), "w") as f:
        f.write(f"[model]\n{body}\n")


# One mock provider per family. A family is what selects a profile, so a
# single mock can only ever exercise one of them; `mock` keeps its old name
# and family so existing invocations still mean what they did.
MOCK_FAMILIES = {"mock": "claude", "mock-claude": "claude", "mock-gpt": "gpt", "mock-codex": "codex"}


def write_mock_providers(config_dir, port, provider, model_id):
    family = MOCK_FAMILIES[provider]
    with open(os.path.join(config_dir, "providers.toml"), "w") as f:
        # `openai-responses`, not chat completions: it is the adapter the GPT
        # family ships on, and the API path is the only one that sends a
        # profile's tool half at all.
        f.write(f"""[providers.{provider}]
name = "Mock {family} (local)"
protocol = "openai-responses"
base_url = "http://127.0.0.1:{port}/v1"
family = "{family}"
[providers.{provider}.auth]
none = true
[[providers.{provider}.models]]
id = "{model_id}"
name = "Mock model"
[providers.{provider}.models.capabilities]
context_window = 32768
max_output_tokens = 4096
tools = true
""")


# Auth crosses the isolation boundary on its own: an engine's login belongs to
# its CLI (`claude`, `codex`), and an API key comes through the inherited
# environment. Nothing here copies a credential file.


def free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def wait_for_port(host, port, timeout):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection((host, port), timeout=0.5):
                return
        except OSError:
            time.sleep(0.1)
    raise RuntimeError(f"mock server did not open port {port} within {timeout}s")


def start_mock_server(root, port):
    log = open(os.path.join(root, "mock.log"), "w")
    proc = subprocess.Popen([sys.executable, MOCK_SERVER, str(port)], stdout=log, stderr=subprocess.STDOUT)
    try:
        wait_for_port("127.0.0.1", port, timeout=10)
    except Exception:
        proc.kill()
        proc.wait(timeout=5)
        log.close()
        raise
    return proc, log


def stop_mock_server(proc, log):
    if proc is None:
        return
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=5)
    log.close()


# ----- driving one task through the daemon protocol -------------------------


def wait_for_terminal(client, agent_id, timeout):
    """Drive the subscription until the agent completes, fails, is
    cancelled, or goes idle on its inbox (a `run`-style single-shot end
    state, mirroring `agent_is_idle` in crates/aether/src/cli.rs)."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            frame = client.next_frame(timeout=max(1.0, deadline - time.time()))
        except OSError:
            break
        if frame.get("frame") != "event":
            continue
        ev = frame["event"]
        if ev.get("agent_id") != agent_id:
            continue
        kind = ev["kind"]
        kind_type = kind["type"]
        if kind_type == "agent.completed":
            return "completed", None
        if kind_type == "agent.failed":
            return "failed", kind.get("error") or "agent failed"
        if kind_type == "agent.cancelled":
            return "cancelled", kind.get("reason") or "agent cancelled"
        if kind_type == "agent.waiting" and kind.get("condition", {}).get("on") == "inbox":
            return "waiting", "agent went idle before completing the objective"
    return "timeout", f"no terminal event within {timeout:.0f}s"


def run_once(binary, model, condition, task, root, mock_port):
    """One (model, condition, task) run: fresh workspace, throwaway isolated
    daemon, one agent turn to completion, then teardown. Success is always
    the task's own `check` verdict, independent of how the agent ended."""
    ws = fresh_workspace(root)
    before_digest = tests_digest(ws)
    env = isolated_env(root)
    write_model_config(env["AETHER_CONFIG_DIR"], condition)

    provider = model.split("/", 1)[0]
    mock_proc = mock_log = None
    client = None
    status = error = None
    stats = {"turns": 0, "tool_calls": 0, "usage": {}}
    try:
        if provider in MOCK_FAMILIES:
            model_id = model.split("/", 1)[1]
            write_mock_providers(env["AETHER_CONFIG_DIR"], mock_port, provider, model_id)
            mock_proc, mock_log = start_mock_server(root, mock_port)

        start_daemon(binary, env, ws)

        client = AetherClient(env["AETHER_SOCKET"])
        client.subscribe()  # before create_agent: the agent can start (and
        # finish) before we would otherwise be listening.
        agent = client.create_agent(role="coder", objective=task["objective"], model=model, long_lived=False)
        agent_id = agent["id"]
        status, error = wait_for_terminal(client, agent_id, RUN_TIMEOUT_SECONDS)
        stats = client.get_agent(agent_id)["stats"]
    finally:
        if client is not None:
            client.sock.close()
        # Unconditional and always last: `start_daemon` can time out waiting
        # for the socket after the process has already forked, so a failed
        # start still needs a stop attempt. `stop_daemon` is a no-op if
        # nothing is listening.
        stop_daemon(binary, env)
        stop_mock_server(mock_proc, mock_log)

    usage = stats.get("usage", {})
    ok, detail = check(ws, task, before_digest)
    return dict(
        model=model,
        condition=condition,
        task=task["id"],
        success=ok,
        status=status,
        error=error,
        turns=stats.get("turns", 0),
        tool_calls=stats.get("tool_calls", 0),
        input_tokens=usage.get("input_tokens", 0),
        output_tokens=usage.get("output_tokens", 0),
        cached_tokens=usage.get("cache_read_tokens", 0),
        cost_usd=usage.get("cost_usd"),
        detail=detail,
    )


# ----- summary and results file ---------------------------------------------


def aggregate(rows):
    n = len(rows)
    costs = [r["cost_usd"] for r in rows if r["cost_usd"] is not None]
    return dict(
        n=n,
        success_rate=sum(r["success"] for r in rows) / n,
        mean_turns=sum(r["turns"] for r in rows) / n,
        mean_tool_calls=sum(r["tool_calls"] for r in rows) / n,
        mean_input_per_turn=sum(r["input_tokens"] / max(1, r["turns"]) for r in rows) / n,
        mean_cost=(sum(costs) / len(costs)) if costs else None,
        mean_seconds=sum(r["seconds"] for r in rows) / n,
    )


def verdict(profile_agg, plain_agg):
    """Outcome decides, and only outcome: success rate first, then mean turns.
    Tool-call counts and tokens are reported but not graded - a profile costs
    tokens by existing, and grading it for that would make every profile
    "hurt". VISION §4.4 asks for "helps or is neutral", which is a question
    about the work done, not about the size of the prompt.
    """
    eps = 1e-9
    if profile_agg["success_rate"] == 0 and plain_agg["success_rate"] == 0:
        # No outcome to compare at all; the tiebreak would grade the profile
        # on its token cost alone, which is not evidence either way.
        return "no signal"
    if abs(profile_agg["success_rate"] - plain_agg["success_rate"]) > eps:
        return "helps" if profile_agg["success_rate"] > plain_agg["success_rate"] else "hurts"
    if abs(profile_agg["mean_turns"] - plain_agg["mean_turns"]) > eps:
        return "helps" if profile_agg["mean_turns"] < plain_agg["mean_turns"] else "hurts"
    return "neutral"


def print_summary(rows):
    print(f"\n{'model':<28} {'cond':<8} {'runs':>4} {'ok%':>5} {'turns':>6} {'tools':>6} {'in/turn':>8} {'cost':>9} {'sec':>7}")
    for model in sorted(set(r["model"] for r in rows)):
        aggs = {}
        for condition in ("profile", "plain"):
            crows = [r for r in rows if r["model"] == model and r["condition"] == condition]
            if not crows:
                continue
            agg = aggregate(crows)
            aggs[condition] = agg
            cost = f"${agg['mean_cost']:.4f}" if agg["mean_cost"] is not None else "n/a"
            print(f"{model:<28} {condition:<8} {agg['n']:>4} {agg['success_rate'] * 100:>4.0f}% {agg['mean_turns']:>6.1f} "
                  f"{agg['mean_tool_calls']:>6.1f} {agg['mean_input_per_turn']:>8.0f} {cost:>9} {agg['mean_seconds']:>7.1f}")
        if "profile" in aggs and "plain" in aggs:
            price = aggs["profile"]["mean_input_per_turn"] - aggs["plain"]["mean_input_per_turn"]
            n = aggs["profile"]["n"]
            print(f"{'':<28} verdict: {verdict(aggs['profile'], aggs['plain'])} "
                  f"(n={n} per condition; the profile costs {price:+.0f} input tokens/turn)")


def binary_version(binary):
    try:
        proc = subprocess.run([binary, "--version"], capture_output=True, text=True, timeout=10)
        return proc.stdout.strip() or None
    except (OSError, subprocess.TimeoutExpired):
        return None


def repo_relative(path):
    """A committed result must not carry one machine's checkout layout, so the
    binary is recorded relative to the repository root."""
    root = os.path.dirname(HERE)
    rel = os.path.relpath(path, root)
    return rel if not rel.startswith(os.pardir) else os.path.basename(path)


def write_results(out_dir, command, binary, rows):
    os.makedirs(out_dir, exist_ok=True)
    # Timestamped, not just dated: two runs on one day are two results, and
    # the second must not quietly replace the first.
    path = os.path.join(out_dir, f"model-profiles-{time.strftime('%Y-%m-%dT%H%M%SZ', time.gmtime())}.json")
    summary = {}
    for model in sorted(set(r["model"] for r in rows)):
        aggs = {c: aggregate([r for r in rows if r["model"] == model and r["condition"] == c])
                for c in ("profile", "plain")
                if any(r["model"] == model and r["condition"] == c for r in rows)}
        if "profile" in aggs and "plain" in aggs:
            aggs["verdict"] = verdict(aggs["profile"], aggs["plain"])
        summary[model] = aggs
    payload = dict(
        command=command,
        generated_at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        binary=repo_relative(binary),
        binary_version=binary_version(binary),
        summary=summary,
        runs=rows,
    )
    with open(path, "w") as f:
        json.dump(payload, f, indent=2)
    return path


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model", action="append", required=True,
                     help="provider/model, e.g. claude-code/haiku; repeatable and/or comma-separated")
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--tasks", default=None, help="comma-separated task ids")
    ap.add_argument("--binary", default=os.path.join(HERE, "..", "target", "release", "aether"))
    ap.add_argument("--out", default=os.path.join(HERE, "results"))
    ap.add_argument("--mock-port", type=int, default=None, help="fixed port for scripts/mock_openai.py (default: an OS-assigned free port)")
    a = ap.parse_args()

    models = [m.strip() for spec in a.model for m in spec.split(",") if m.strip()]
    for m in models:
        provider = m.split("/", 1)[0]
        if provider.startswith("mock") and provider not in MOCK_FAMILIES:
            raise SystemExit(f"unknown mock provider {provider!r}: one of {sorted(MOCK_FAMILIES)}")

    with open(os.path.join(HERE, "tasks.json")) as f:
        tasks = json.load(f)
    if a.tasks:
        wanted = set(a.tasks.split(","))
        tasks = [t for t in tasks if t["id"] in wanted]
    if not tasks:
        raise SystemExit("no matching tasks")

    binary = os.path.realpath(a.binary)
    mock_port = a.mock_port or free_port()
    command = "python3 evals/model_profiles.py " + " ".join(shlex.quote(x) for x in sys.argv[1:])

    rows = []
    for model in models:
        for task in tasks:
            for run in range(1, a.runs + 1):
                for condition in ("profile", "plain"):
                    root = tempfile.mkdtemp(prefix=f"aether-eval-{task['id']}-")
                    print(f"-> {model} {condition} {task['id']} run {run} ({root})", flush=True)
                    started = time.time()
                    row = run_once(binary, model, condition, task, root, mock_port)
                    # The run root is printed, not recorded: a committed
                    # result should not carry one machine's temp paths.
                    row.update(run=run, seconds=round(time.time() - started, 1), at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()))
                    rows.append(row)
                    cost = f"${row['cost_usd']:.4f}" if row["cost_usd"] is not None else "n/a"
                    print(f"   success={row['success']} status={row['status']} turns={row['turns']} tools={row['tool_calls']} "
                          f"in={row['input_tokens']} out={row['output_tokens']} cached={row['cached_tokens']} cost={cost} "
                          f"{row['seconds']}s  {row['detail'][:80]}", flush=True)

    print_summary(rows)
    path = write_results(a.out, command, binary, rows)
    print(f"\nresults written to {path}")


if __name__ == "__main__":
    main()
