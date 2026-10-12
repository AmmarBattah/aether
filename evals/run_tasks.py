#!/usr/bin/env python3
"""Run the task set through Aether and through the native reference agent and
compare success, turns, tool calls, tokens, cost and wall time.

Usage:
  evals/.venv/bin/python evals/run_tasks.py --model anthropic/claude-opus-5-5 [--runs 2] [--tasks fix_bug,readme]
                                            [--binary target/release/aether] [--effort high]

Requires ANTHROPIC_API_KEY (or another configured provider for Aether; the
reference agent always uses the Anthropic SDK). Results append to
evals/results/tasks.jsonl and a summary table prints at the end.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from aether_client import isolated_env, start_daemon, stop_daemon  # noqa: E402

FIXTURE = os.path.join(HERE, "fixtures", "pyproj")
RESULTS = os.path.join(HERE, "results")
USAGE_RE = re.compile(r"usage: (\d+) in · (\d+) out · (\d+) cached · (\d+) turns · \$([0-9.]+)")


def fresh_workspace(root):
    ws = os.path.join(root, "workspace")
    shutil.copytree(FIXTURE, ws)
    return ws


def tests_digest(ws):
    h = hashlib.sha256()
    for name in sorted(os.listdir(os.path.join(ws, "tests"))):
        with open(os.path.join(ws, "tests", name), "rb") as f:
            h.update(name.encode() + f.read())
    return h.hexdigest()


def check(ws, task, before_digest):
    if task.get("tests_frozen") and tests_digest(ws) != before_digest:
        return False, "tests were modified"
    proc = subprocess.run(["bash", "-c", task["check"]], cwd=ws, capture_output=True, text=True, timeout=120,
                          env={**os.environ, "PATH": os.path.dirname(sys.executable) + os.pathsep + os.environ["PATH"]})
    return proc.returncode == 0, (proc.stdout + proc.stderr)[-300:]


def run_aether(binary, model, effort, task, root):
    ws = fresh_workspace(root)
    env = isolated_env(root)
    start_daemon(binary, env, ws)
    digest = tests_digest(ws)
    started = time.time()
    cmd = [binary, "run", "--yes", "--model", model, "--role", "coder"]
    if effort:
        cmd += ["--effort", effort]
    cmd.append(task["objective"])
    try:
        proc = subprocess.run(cmd, env=env, cwd=ws, capture_output=True, text=True, timeout=1800)
    finally:
        stop_daemon(binary, env)
    seconds = time.time() - started
    m = USAGE_RE.search(proc.stderr)
    usage = dict(input=int(m.group(1)), output=int(m.group(2)), cache_read=int(m.group(3)), turns=int(m.group(4)),
                 cost=float(m.group(5))) if m else dict(input=0, output=0, cache_read=0, turns=0, cost=0.0)
    usage["tool_calls"] = proc.stderr.count("\n▸ ") + proc.stderr.startswith("▸ ")
    ok, detail = check(ws, task, digest)
    return dict(harness="aether", ok=ok, detail=detail, seconds=round(seconds, 1), stderr_tail=proc.stderr[-800:], **usage)


def run_reference(model, effort, task, root):
    ws = fresh_workspace(root)
    digest = tests_digest(ws)
    name = model.split("/", 1)[1] if "/" in model else model
    cmd = [sys.executable, os.path.join(HERE, "reference_agent.py"), "--workspace", ws, "--model", name]
    if effort:
        cmd += ["--effort", effort]
    cmd.append(task["objective"])
    started = time.time()
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=1800)
    seconds = time.time() - started
    try:
        usage = json.loads(proc.stdout.strip().splitlines()[-1])
    except (json.JSONDecodeError, IndexError):
        usage = dict(input=0, output=0, cache_read=0, cache_write=0, turns=0, tool_calls=0, final=proc.stderr[-500:])
    ok, detail = check(ws, task, digest)
    return dict(harness="reference", ok=ok, detail=detail, seconds=round(seconds, 1), cost=None,
                input=usage.get("input", 0), output=usage.get("output", 0), cache_read=usage.get("cache_read", 0),
                turns=usage.get("turns", 0), tool_calls=usage.get("tool_calls", 0), stderr_tail=proc.stderr[-800:])


def summarize(rows):
    by = {}
    for r in rows:
        by.setdefault((r["task"], r["harness"]), []).append(r)
    print(f"\n{'task':<14} {'harness':<10} {'runs':>4} {'ok':>4} {'turns':>6} {'tools':>6} {'in':>9} {'out':>8} {'cached':>9} {'sec':>7}")
    for (task, harness), rs in sorted(by.items()):
        n = len(rs)
        avg = lambda k: sum(r.get(k) or 0 for r in rs) / n  # noqa: E731
        print(f"{task:<14} {harness:<10} {n:>4} {sum(r['ok'] for r in rs):>4} {avg('turns'):>6.1f} {avg('tool_calls'):>6.1f} "
              f"{avg('input'):>9.0f} {avg('output'):>8.0f} {avg('cache_read'):>9.0f} {avg('seconds'):>7.1f}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="anthropic/claude-opus-5-5")
    ap.add_argument("--effort", default=None)
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--tasks", default=None, help="comma-separated task ids")
    ap.add_argument("--binary", default=os.path.join(HERE, "..", "target", "release", "aether"))
    ap.add_argument("--only", choices=["aether", "reference"], default=None)
    a = ap.parse_args()
    with open(os.path.join(HERE, "tasks.json")) as f:
        tasks = json.load(f)
    if a.tasks:
        wanted = set(a.tasks.split(","))
        tasks = [t for t in tasks if t["id"] in wanted]
    os.makedirs(RESULTS, exist_ok=True)
    out_path = os.path.join(RESULTS, "tasks.jsonl")
    rows = []
    binary = os.path.realpath(a.binary)
    for task in tasks:
        for run in range(a.runs):
            for harness in ("aether", "reference"):
                if a.only and harness != a.only:
                    continue
                root = tempfile.mkdtemp(prefix=f"aether-eval-{task['id']}-")
                print(f"→ {task['id']} run {run + 1} via {harness} ({root})", flush=True)
                try:
                    row = run_aether(binary, a.model, a.effort, task, root) if harness == "aether" else run_reference(a.model, a.effort, task, root)
                except Exception as e:  # noqa: BLE001 - record and continue with the next run
                    row = dict(harness=harness, ok=False, detail=f"crashed: {e}", seconds=0, input=0, output=0, cache_read=0, turns=0, tool_calls=0, cost=None, stderr_tail="")
                row.update(task=task["id"], model=a.model, effort=a.effort, run=run + 1, root=root, at=time.strftime("%Y-%m-%dT%H:%M:%S"))
                rows.append(row)
                with open(out_path, "a") as f:
                    f.write(json.dumps(row) + "\n")
                print(f"   ok={row['ok']} turns={row['turns']} tools={row['tool_calls']} in={row['input']} out={row['output']} cached={row['cache_read']} {row['seconds']}s  {row['detail'][:80]}", flush=True)
    summarize(rows)
    print(f"\nraw rows appended to {out_path}")


if __name__ == "__main__":
    main()
