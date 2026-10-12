#!/usr/bin/env python3
"""A minimal "native Claude" coding agent: the official SDK, a plain manual
tool loop, and the same tools Aether exposes. It is the baseline Aether is
compared against, so it stays deliberately simple and fair: same model, same
workspace, prompt caching on the stable prefix, no harness features.

Usage: python3 evals/reference_agent.py --workspace DIR --model claude-opus-5-5 "objective"
Prints a JSON usage summary on the last line of stdout.
"""

import argparse
import json
import os
import subprocess
import sys
import time

import anthropic

SYSTEM = (
    "You are a coding agent working in a repository at the given workspace. "
    "Use the tools to inspect and change files and to run commands. "
    "When the task is complete, reply with a short summary and stop calling tools."
)

TOOLS = [
    {
        "name": "bash",
        "description": "Run a shell command in the workspace and return its output.",
        "input_schema": {
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"],
            "additionalProperties": False,
        },
        "strict": True,
    },
    {
        "name": "read_file",
        "description": "Read a file relative to the workspace.",
        "input_schema": {
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"],
            "additionalProperties": False,
        },
        "strict": True,
    },
    {
        "name": "write_file",
        "description": "Create or overwrite a file relative to the workspace.",
        "input_schema": {
            "type": "object",
            "properties": {"path": {"type": "string"}, "content": {"type": "string"}},
            "required": ["path", "content"],
            "additionalProperties": False,
        },
        "strict": True,
    },
    {
        "name": "edit_file",
        "description": "Replace an exact substring in a file (must match exactly once).",
        "input_schema": {
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "old_string": {"type": "string"},
                "new_string": {"type": "string"},
            },
            "required": ["path", "old_string", "new_string"],
            "additionalProperties": False,
        },
        "strict": True,
    },
]


def resolve(workspace, path):
    full = os.path.realpath(os.path.join(workspace, path))
    if not full.startswith(os.path.realpath(workspace) + os.sep) and full != os.path.realpath(workspace):
        raise ValueError(f"path escapes workspace: {path}")
    return full


def run_tool(workspace, name, inp):
    try:
        if name == "bash":
            proc = subprocess.run(
                ["bash", "-c", inp["command"]], cwd=workspace, capture_output=True, text=True, timeout=120
            )
            out = (proc.stdout + proc.stderr)[-30_000:]
            return f"{out}\n[exit code {proc.returncode}]", proc.returncode != 0
        if name == "read_file":
            with open(resolve(workspace, inp["path"]), encoding="utf-8", errors="replace") as f:
                return f.read()[:30_000], False
        if name == "write_file":
            full = resolve(workspace, inp["path"])
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "w", encoding="utf-8") as f:
                f.write(inp["content"])
            return f"wrote {len(inp['content'])} bytes to {inp['path']}", False
        if name == "edit_file":
            full = resolve(workspace, inp["path"])
            with open(full, encoding="utf-8") as f:
                text = f.read()
            n = text.count(inp["old_string"])
            if n != 1:
                return f"old_string must match exactly once, found {n}", True
            with open(full, "w", encoding="utf-8") as f:
                f.write(text.replace(inp["old_string"], inp["new_string"]))
            return "edited", False
        return f"unknown tool {name}", True
    except Exception as e:  # noqa: BLE001 - surface every tool failure to the model
        return f"error: {e}", True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("objective")
    ap.add_argument("--workspace", required=True)
    ap.add_argument("--model", default="claude-opus-5-5")
    ap.add_argument("--effort", default=None)
    ap.add_argument("--max-turns", type=int, default=40)
    a = ap.parse_args()

    client = anthropic.Anthropic()
    messages = [{"role": "user", "content": a.objective}]
    usage = {"input": 0, "output": 0, "cache_read": 0, "cache_write": 0, "turns": 0, "tool_calls": 0}
    started = time.time()
    final_text = ""
    for _ in range(a.max_turns):
        kwargs = dict(
            model=a.model,
            max_tokens=16000,
            system=[{"type": "text", "text": SYSTEM, "cache_control": {"type": "ephemeral"}}],
            tools=TOOLS,
            messages=messages,
        )
        if a.effort:
            kwargs["output_config"] = {"effort": a.effort}
        with client.messages.stream(**kwargs) as stream:
            response = stream.get_final_message()
        usage["turns"] += 1
        usage["input"] += response.usage.input_tokens
        usage["output"] += response.usage.output_tokens
        usage["cache_read"] += response.usage.cache_read_input_tokens or 0
        usage["cache_write"] += response.usage.cache_creation_input_tokens or 0
        messages.append({"role": "assistant", "content": response.content})
        tool_uses = [b for b in response.content if b.type == "tool_use"]
        final_text = "".join(b.text for b in response.content if b.type == "text")
        if response.stop_reason == "refusal":
            final_text = "[refused]"
            break
        if not tool_uses:
            break
        results = []
        for tu in tool_uses:
            usage["tool_calls"] += 1
            content, is_error = run_tool(a.workspace, tu.name, tu.input)
            print(f"▸ {tu.name} {json.dumps(tu.input)[:120]}", file=sys.stderr)
            results.append({"type": "tool_result", "tool_use_id": tu.id, "content": content, "is_error": is_error})
        messages.append({"role": "user", "content": results})
    usage["seconds"] = round(time.time() - started, 2)
    usage["final"] = final_text[:500]
    print(json.dumps(usage))


if __name__ == "__main__":
    main()
