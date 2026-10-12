"""Independent, allowlisted Claude process receipts for native-harness benchmarks."""
import json
import os


# ----- transparent PATH proxy for native Claude receipts --------------------
#
# The Aether adapter's durable stats can omit auxiliary ("helper") models
# that Claude's own modelUsage reports (see evals/harness_bench_usage.py).
# To audit that gap without touching the daemon under test, every Aether
# trial's isolated PATH gets a `claude` shim that execs the real binary with
# its output completely unchanged, and separately records the
# final `result` event (or nothing, if the process never produced one) per
# invocation. Modeled on evals/claude_context.py's PROXY. Its arguments are
# unchanged too, but for one optional flag: with `strict_mcp` a print-mode
# launch also gets --strict-mcp-config, so the session loads the MCP servers
# of Aether's own --mcp-config and none of the machine user's.
_CLAUDE_PROXY = '''#!/usr/bin/env python3
import json, os, subprocess, sys, time
from pathlib import Path
real = {real!r}
arguments = sys.argv[1:]
if not any(arg in ("-p", "--print") for arg in arguments):
    os.execv(real, [real, *arguments])
strict_mcp = {strict_mcp!r} and "--strict-mcp-config" not in arguments
if strict_mcp:
    arguments = ["--strict-mcp-config", *arguments]
receipt_path = Path({receipts!r}) / (str(os.getpid()) + ".json")
started_ns = time.monotonic_ns()
resumed = any(arg in ("--resume", "-r", "--continue", "-c")
              or arg.startswith("--resume=") for arg in arguments)

def save_receipt(receipt):
    temporary = receipt_path.with_suffix(".tmp")
    temporary.write_text(json.dumps({{"capture_id": str(os.getpid()), "receipt": receipt,
                                    "started_ns": started_ns, "resumed": resumed,
                                    "strict_mcp_added": strict_mcp}}))
    os.replace(temporary, receipt_path)

def counters(raw):
    if not isinstance(raw, dict):
        return None
    allowed = ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens",
               "inputTokens", "outputTokens", "cacheReadInputTokens", "cacheCreationInputTokens",
               "thinkingTokens", "costUSD", "contextWindow", "maxOutputTokens", "webSearchRequests")
    result = {{key: raw[key] for key in allowed if key in raw}}
    details = raw.get("output_tokens_details")
    if isinstance(details, dict) and "thinking_tokens" in details:
        result["output_tokens_details"] = {{"thinking_tokens": details["thinking_tokens"]}}
    return result

def capture_result(line):
    try:
        event = json.loads(line)
    except (ValueError, UnicodeDecodeError):
        return
    if not isinstance(event, dict) or event.get("type") != "result":
        return
    if isinstance(event.get("origin"), dict) and event["origin"].get("kind") == "task-notification":
        return
    receipt = {{key: event[key] for key in ("type", "subtype", "is_error", "session_id", "total_cost_usd",
                                          "num_turns", "duration_ms", "duration_api_ms") if key in event}}
    if "usage" in event:
        receipt["usage"] = counters(event["usage"])
    if isinstance(event.get("modelUsage"), dict):
        receipt["modelUsage"] = {{model: counters(usage) for model, usage in event["modelUsage"].items()}}
    save_receipt(receipt)

save_receipt(None)
child = subprocess.Popen([real, *arguments], stdout=subprocess.PIPE)
try:
    for line in child.stdout:
        # The adapter can terminate this process as soon as it sees a result.
        capture_result(line)
        sys.stdout.buffer.write(line)
        sys.stdout.buffer.flush()
    code = child.wait()
finally:
    child.stdout.close()
    if child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=5)
sys.exit(code)
'''


def install_claude_receipt_proxy(env, trial_dir, real_claude, strict_mcp=False):
    """Prepend a transparent `claude` shim to `env["PATH"]`; returns the
    receipts directory. A no-op-content directory is fine (nothing spawned
    `claude`) and is not itself a failure. `strict_mcp` keeps the machine
    user's MCP servers out of every session the shim launches."""
    assert real_claude, "the real `claude` CLI must be resolvable to proxy it"
    assert isinstance(strict_mcp, bool), "strict_mcp is a yes or a no"
    wrappers, receipts = os.path.join(trial_dir, "bin"), os.path.join(trial_dir, "claude-receipts")
    os.makedirs(wrappers, exist_ok=True)
    os.makedirs(receipts, exist_ok=True)
    proxy_path = os.path.join(wrappers, "claude")
    with open(proxy_path, "w") as f:
        f.write(_CLAUDE_PROXY.format(real=real_claude, receipts=receipts, strict_mcp=strict_mcp))
    os.chmod(proxy_path, 0o755)
    env["PATH"] = wrappers + os.pathsep + env["PATH"]
    return receipts


def read_claude_receipts(receipts_dir):
    captures = []
    for name in sorted(os.listdir(receipts_dir)):
        if name.endswith(".json"):
            with open(os.path.join(receipts_dir, name)) as f:
                captures.append(json.load(f))
    return captures


def attribute_claude_receipts(condition, root_id, captures):
    """Only aether-single-claude has an unambiguous 1:1 process-to-agent
    mapping (the sole agent in that isolated daemon); every other condition
    records agent_id=None, an intentionally unattributed capture."""
    single_claude = condition["kind"] == "aether-single" and condition["model"].startswith("claude-code/")
    return [
        {**c, "agent_id": root_id if single_claude else None}
        for c in captures
    ]
