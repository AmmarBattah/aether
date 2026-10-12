#!/usr/bin/env python3
"""Per-trial figures a manifest does not hold, read from the solver's own session transcript.

For every trial of the given manifests, finds the Claude Code or Codex session
that ran in the trial's workspace and reports: the first request's input tokens,
thinking (or reasoning) tokens before the first edit and in total, output
tokens, API requests, tool calls by kind, test runs, the tests the solver says
it added, and the seconds from its last change to the end. Read-only. A figure
the transcript does not record is null, never zero, and a trial whose transcript
cannot be found says why.

An edit is a file-editing tool call (Claude: Edit, Write, MultiEdit,
NotebookEdit; Codex: a FileChange item). A file written from a shell command is
not seen as an edit. For both CLIs "before the first edit" runs through the
response that issued that edit, so its own thinking or reasoning is counted:
the solver thinks and then writes inside one response. Claude Code records the tools it loaded with the first
request; a Codex rollout does not, so that figure is null for Codex, which
instead gets the size of each block of context its first turn was given.

Usage:
  python3 evals/harness_bench_transcript.py MANIFEST [MANIFEST ...] [--output FILE]
"""
import argparse
import json
import re
import sys
from datetime import datetime
from pathlib import Path

CLAUDE_EDIT_TOOLS = ("Edit", "Write", "MultiEdit", "NotebookEdit")
CODEX_TOOL_ITEMS_EXCLUDED = ("UserMessage", "AgentMessage", "Reasoning")
TEST_COMMAND = re.compile(r"\b(pytest|unittest)\b")
TESTS_RUN = (re.compile(r"\bRan (\d+) tests?\b"), re.compile(r"\b(\d+) passed\b"))
TESTS_STATED = (re.compile(r"\badd(?:ed|s|ing)?\s+(\d+)\s+(?:[\w-]+\s+){0,2}tests?\b", re.IGNORECASE),
                re.compile(r"\b(\d+)\s+(?:new|additional|more|extra)\s+(?:[\w-]+\s+)?tests?\b", re.IGNORECASE),
                re.compile(r"\b(\d+)\s+tests?(?:\s+cases)?\s+added\b", re.IGNORECASE))
CODEX_HEADER_LINES = 40
CODEX_CONTEXT_LABEL = 28


class TranscriptError(ValueError):
    """The transcript is missing, ambiguous or not in the expected shape."""


def read_jsonl(path):
    rows = [json.loads(line) for line in Path(path).read_text(encoding="utf-8").splitlines() if line.strip()]
    if not rows or not all(isinstance(row, dict) for row in rows):
        raise TranscriptError(f"{Path(path).name}: not a JSON-lines transcript")
    return rows


def _seconds_between(start, end):
    """Seconds between two ISO timestamps; None when either is absent."""
    if not start or not end:
        return None
    parsed = [datetime.fromisoformat(stamp.replace("Z", "+00:00")) for stamp in (start, end)]
    seconds = (parsed[1] - parsed[0]).total_seconds()
    assert seconds >= 0, f"a transcript's end precedes its last change: {start} > {end}"
    return round(seconds, 3)


def _first_number(patterns, text):
    for pattern in patterns:
        found = pattern.search(text or "")
        if found:
            return int(found.group(1))
    return None


def _sum_known(values):
    """A total only when every request reported the figure; otherwise unknown."""
    return None if any(value is None for value in values) else sum(values)


def _all_text(parts):
    return isinstance(parts, list) and all(isinstance(part, str) for part in parts)


def _largest(values):
    """The 1-based position and size of the largest value; (None, None) when any is unrecorded."""
    if not values or any(value is None for value in values):
        return None, None
    position = max(range(len(values)), key=values.__getitem__)
    return position + 1, values[position]


def _input_split(uncached, cache_read, cache_write):
    assert all(type(value) is int and value >= 0 for value in (uncached, cache_read, cache_write)), \
        f"impossible input counters: {(uncached, cache_read, cache_write)}"
    return {"uncached": uncached, "cache_read": cache_read, "cache_write": cache_write,
            "total": uncached + cache_read + cache_write}


# ----- Claude Code ----------------------------------------------------------


def claude_requests(rows):
    """One entry per API response of the main chain: its usage, content blocks and first timestamp."""
    requests, by_id = [], {}
    for row in rows:
        message = row.get("message") or {}
        if row.get("type") != "assistant" or row.get("isSidechain") or message.get("model") == "<synthetic>":
            continue
        key = message.get("id") or row.get("uuid")
        if key not in by_id:
            by_id[key] = {"blocks": [], "timestamp": row.get("timestamp"), "usage": None}
            requests.append(by_id[key])
        # Every row of one response repeats its usage; the last row carries the final counters.
        by_id[key]["usage"] = message.get("usage") or by_id[key]["usage"]
        by_id[key]["blocks"].extend((block, row.get("timestamp")) for block in message.get("content") or [])
    if not requests:
        raise TranscriptError("no assistant response in the Claude transcript")
    if any(not isinstance(request["usage"], dict) for request in requests):
        raise TranscriptError("a Claude response carries no usage")
    return requests


def _claude_tool_results(rows):
    """Tool-use id -> the text the tool returned."""
    results = {}
    for row in rows:
        content = (row.get("message") or {}).get("content")
        if row.get("type") != "user" or not isinstance(content, list):
            continue
        for block in content:
            if block.get("type") != "tool_result":
                continue
            body = block.get("content")
            if isinstance(body, list):
                body = "".join(part.get("text", "") for part in body if isinstance(part, dict))
            results[block.get("tool_use_id")] = body if isinstance(body, str) else ""
    return results


def _claude_context(rows):
    """What the session's first turn was given, where the transcript records it."""
    deferred = instructions = loaded = None
    for row in rows:
        attachment = row.get("attachment") or {}
        if attachment.get("type") == "deferred_tools_delta" and deferred is None:
            deferred = len(attachment.get("addedNames") or [])
        if attachment.get("type") == "mcp_instructions_delta" and instructions is None:
            instructions = sum(len(block) for block in attachment.get("addedBlocks") or [])
        if attachment.get("type") == "prompt_snapshot" and isinstance(attachment.get("tools"), list) and loaded is None:
            loaded = attachment
    first_user = next((row for row in rows if row.get("type") == "user"), None)
    content = (first_user or {}).get("message", {}).get("content")
    if isinstance(content, list):
        content = "".join(block.get("text", "") for block in content if block.get("type") == "text")
    tools = (loaded or {}).get("tools")
    system = (loaded or {}).get("systemPrompt")
    assert tools is None or all(isinstance(tool, dict) and tool.get("name") for tool in tools), "a loaded tool is named"
    return {"always_loaded_tools": None if tools is None else sorted(tool["name"] for tool in tools),
            "always_loaded_tool_chars": None if tools is None else sum(len(json.dumps(tool)) for tool in tools),
            "system_prompt_chars": sum(len(part) for part in system) if _all_text(system) else None,
            "deferred_tools": deferred, "mcp_instruction_chars": instructions,
            "first_user_chars": len(content) if isinstance(content, str) else None,
            "first_turn_context_chars": None}


def claude_figures(rows):
    """The figures of one Claude Code session transcript. Thinking before the first edit counts every
    request up to the one that issued the first file edit, that response's own thinking included."""
    requests = claude_requests(rows)
    results = _claude_tool_results(rows)
    tool_calls, test_runs, last_tests, first_edit, last_change = {}, 0, None, None, None
    for index, request in enumerate(requests, start=1):
        for block, stamp in request["blocks"]:
            if block.get("type") != "tool_use":
                continue
            name = block.get("name")
            tool_calls[name] = tool_calls.get(name, 0) + 1
            if name in CLAUDE_EDIT_TOOLS:
                first_edit, last_change = first_edit or index, stamp
            if name == "Bash" and TEST_COMMAND.search(str((block.get("input") or {}).get("command", ""))):
                test_runs += 1
                last_tests = _first_number(TESTS_RUN, results.get(block.get("id"))) or last_tests
    usages = [request["usage"] for request in requests]
    thinking = [(usage.get("output_tokens_details") or {}).get("thinking_tokens") for usage in usages]
    inputs = [_input_split(usage.get("input_tokens", 0), usage.get("cache_read_input_tokens", 0),
                           usage.get("cache_creation_input_tokens", 0)) for usage in usages]
    final_text = "".join(block.get("text", "") for block, _ in requests[-1]["blocks"] if block.get("type") == "text")
    stamps = [row["timestamp"] for row in rows if row.get("timestamp")]
    largest_request, largest_thinking = _largest(thinking)
    assert len(inputs) == len(requests) and inputs[0]["total"] > 0, "the first request must carry input"
    assert first_edit is None or 1 <= first_edit <= len(requests), "the first edit lies inside the session"
    return {
        "kind": "claude", "requests": len(requests), "first_request_input_tokens": inputs[0],
        "request_input_tokens": [split["total"] for split in inputs],
        "uncached_input_tokens": sum(split["uncached"] for split in inputs),
        "cache_read_tokens": sum(split["cache_read"] for split in inputs),
        "cache_write_tokens": sum(split["cache_write"] for split in inputs),
        "output_tokens": sum(usage.get("output_tokens", 0) for usage in usages),
        "thinking_tokens_total": _sum_known(thinking), "first_edit_request": first_edit,
        "thinking_tokens_before_first_edit": _sum_known(thinking[:first_edit]) if first_edit else None,
        "largest_thinking_request": largest_request, "largest_thinking_tokens": largest_thinking,
        "tool_calls": dict(sorted(tool_calls.items())), "test_runs": test_runs, "last_test_run_tests": last_tests,
        "tests_stated_added": _first_number(TESTS_STATED, final_text),
        "seconds_from_last_change_to_end": _seconds_between(last_change, stamps[-1] if stamps else None),
        **_claude_context(rows),
    }


# ----- Codex ----------------------------------------------------------------


def _codex_usage(payload, running_total):
    """One request's counters from a token_count event; None for an event that reports no new request.

    Codex can repeat an event: one whose running total has not moved is the same request again."""
    info = (payload.get("info") or {}) if payload.get("type") == "token_count" else {}
    usage = info.get("last_token_usage")
    if not isinstance(usage, dict) or (running_total is not None and info.get("total_token_usage") == running_total):
        return None
    cached = usage.get("cached_input_tokens", 0)
    assert usage.get("input_tokens", 0) >= cached, "Codex input tokens include the cached ones"
    return {"input": _input_split(usage["input_tokens"] - cached, cached, usage.get("cache_write_input_tokens", 0)),
            "output": usage.get("output_tokens", 0), "reasoning": usage.get("reasoning_output_tokens")}


def _codex_first_turn_context(rows):
    """Characters of each block of context sent before the first request, keyed by the block's opening words."""
    sizes = {}
    for row in rows:
        payload = row.get("payload") or {}
        if payload.get("type") == "token_count":
            break
        if payload.get("type") != "message" or payload.get("role") not in ("developer", "user"):
            continue
        for part in payload.get("content") or []:
            text = part.get("text") or ""
            opening = text.strip().splitlines()[0][:CODEX_CONTEXT_LABEL] if text.strip() else ""
            label = f"{payload['role']}: {opening}"
            sizes[label] = sizes.get(label, 0) + len(text)
    return sizes or None


def codex_figures(rows):
    """The figures of one Codex rollout. Reasoning before the first edit counts every request
    up to the one that issued the first file change."""
    usages, tool_calls, test_runs, last_tests = [], {}, 0, None
    first_edit = last_change = final_text = running_total = None
    for row in rows:
        payload = row.get("payload") or {}
        usage = _codex_usage(payload, running_total)
        if usage is not None:
            usages.append(usage)
            running_total = payload["info"].get("total_token_usage")
        item = payload.get("item") if payload.get("type") == "item_completed" else None
        if not isinstance(item, dict):
            continue
        kind = item.get("type")
        if kind == "AgentMessage":
            final_text = "".join(part.get("text", "") for part in item.get("content") or [])
        if kind in CODEX_TOOL_ITEMS_EXCLUDED:
            continue
        tool_calls[kind] = tool_calls.get(kind, 0) + 1
        if kind == "FileChange":
            first_edit, last_change = first_edit or len(usages) + 1, row.get("timestamp")
        if kind == "CommandExecution" and TEST_COMMAND.search(" ".join(map(str, item.get("command") or []))):
            test_runs += 1
            last_tests = _first_number(TESTS_RUN, item.get("aggregated_output")) or last_tests
    if not usages:
        raise TranscriptError("no token_count event in the Codex transcript")
    reasoning = [usage["reasoning"] for usage in usages]
    stamps = [row["timestamp"] for row in rows if row.get("timestamp")]
    largest_request, largest_thinking = _largest(reasoning)
    assert usages[0]["input"]["total"] > 0, "the first request must carry input"
    assert first_edit is None or first_edit >= 1, "the first edit lies inside the session"
    return {
        "kind": "codex", "requests": len(usages), "first_request_input_tokens": usages[0]["input"],
        "request_input_tokens": [usage["input"]["total"] for usage in usages],
        "uncached_input_tokens": sum(usage["input"]["uncached"] for usage in usages),
        "cache_read_tokens": sum(usage["input"]["cache_read"] for usage in usages),
        "cache_write_tokens": sum(usage["input"]["cache_write"] for usage in usages),
        "output_tokens": sum(usage["output"] for usage in usages),
        "thinking_tokens_total": _sum_known(reasoning), "first_edit_request": first_edit,
        "thinking_tokens_before_first_edit": _sum_known(reasoning[:first_edit]) if first_edit else None,
        "largest_thinking_request": largest_request, "largest_thinking_tokens": largest_thinking,
        "tool_calls": dict(sorted(tool_calls.items())), "test_runs": test_runs, "last_test_run_tests": last_tests,
        "tests_stated_added": _first_number(TESTS_STATED, final_text),
        "seconds_from_last_change_to_end": _seconds_between(last_change, stamps[-1] if stamps else None),
        "always_loaded_tools": None, "always_loaded_tool_chars": None, "system_prompt_chars": None,
        "deferred_tools": None, "mcp_instruction_chars": None, "first_user_chars": None,
        "first_turn_context_chars": _codex_first_turn_context(rows),
    }


# ----- finding a trial's transcript -----------------------------------------


def find_claude_transcript(workspace, claude_home):
    """Claude Code keeps a session under projects/<workspace path, every non-alphanumeric as a dash>."""
    project = Path(claude_home) / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(workspace))
    sessions = sorted(project.glob("*.jsonl"))
    if len(sessions) != 1:
        raise TranscriptError(f"expected one Claude session for the workspace, found {len(sessions)}")
    return sessions[0]


def codex_sessions_by_workspace(codex_home):
    """Working directory -> rollout files, from each rollout's first turn (or, failing that, its session)."""
    index = {}
    for path in sorted((Path(codex_home) / "sessions").glob("*/*/*/*.jsonl")):
        session_cwd = turn_cwd = None
        with path.open(encoding="utf-8") as stream:
            for _, line in zip(range(CODEX_HEADER_LINES), stream):
                row = json.loads(line)
                payload = row.get("payload") or {}
                if row.get("type") == "session_meta":
                    session_cwd = payload.get("cwd")
                if row.get("type") == "turn_context":
                    turn_cwd = payload.get("cwd")
                    break
        index.setdefault(turn_cwd or session_cwd, []).append(path)
    return index


def trial_figures(trial, claude_home, codex_index):
    """One trial's identity and transcript figures, or the reason they could not be read."""
    assert isinstance(trial.get("trial_dir"), str) and trial.get("condition"), "a trial names its directory and cell"
    workspace = str(Path(trial["trial_dir"]) / "workspace")
    model = str((trial.get("requested_treatment") or {}).get("model", ""))
    is_codex = trial.get("requested_treatment", {}).get("kind") == "native-codex" or model.startswith("codex/")
    row = {key: trial.get(key) for key in ("condition", "case", "run", "short_id")}
    try:
        if is_codex:
            sessions = codex_index.get(workspace, [])
            if len(sessions) != 1:
                raise TranscriptError(f"expected one Codex session for the workspace, found {len(sessions)}")
            figures = codex_figures(read_jsonl(sessions[0]))
        else:
            figures = claude_figures(read_jsonl(find_claude_transcript(workspace, claude_home)))
    except (TranscriptError, OSError, json.JSONDecodeError) as error:
        return {**row, "transcript_error": f"{type(error).__name__}: {error}"}
    assert figures["requests"] == len(figures["request_input_tokens"]), "one input figure per request"
    return {**row, **figures}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("manifests", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, help="write the JSON here instead of stdout")
    parser.add_argument("--claude-home", type=Path, default=Path.home() / ".claude")
    parser.add_argument("--codex-home", type=Path, default=Path.home() / ".codex")
    args = parser.parse_args(argv)
    codex_index = codex_sessions_by_workspace(args.codex_home)
    report = []
    for path in args.manifests:
        manifest = json.loads(path.read_text())
        if not isinstance(manifest.get("trials"), list):
            raise ValueError(f"{path}: not a harness-bench manifest (no trials list)")
        report.extend({"manifest": path.name, "seed": manifest.get("seed"),
                       **trial_figures(trial, args.claude_home, codex_index)} for trial in manifest["trials"])
    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(text)
    else:
        sys.stdout.write(text)
    return report


if __name__ == "__main__":
    main()
