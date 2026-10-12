"""The transcript reader on small synthetic sessions; no real transcript is read or kept."""
import json
import tempfile
import unittest
from pathlib import Path

import harness_bench_transcript as transcript

WORKSPACE = "/work/tree/qi_1/target/hb/abc123/workspace"


def _usage(cache_read, cache_write, output, thinking=None):
    usage = {"input_tokens": 2, "cache_read_input_tokens": cache_read,
             "cache_creation_input_tokens": cache_write, "output_tokens": output}
    if thinking is not None:
        usage["output_tokens_details"] = {"thinking_tokens": thinking}
    return usage


def _assistant(message_id, stamp, usage, *blocks, **extra):
    return {"type": "assistant", "timestamp": stamp, **extra,
            "message": {"id": message_id, "model": "claude-haiku-5-5", "usage": usage, "content": list(blocks)}}


def _tool(name, tool_id, **tool_input):
    return {"type": "tool_use", "name": name, "id": tool_id, "input": tool_input}


def _result(tool_id, text, stamp):
    return {"type": "user", "timestamp": stamp,
            "message": {"content": [{"type": "tool_result", "tool_use_id": tool_id, "content": text}]}}


def claude_rows():
    """Three requests: read, plan and write, then test and answer; the middle one spans two rows."""
    plan = _usage(1500, 100, 900, thinking=700)
    return [
        {"type": "attachment", "attachment": {"type": "deferred_tools_delta", "addedNames": ["a", "b", "c"]}},
        {"type": "attachment", "attachment": {"type": "mcp_instructions_delta", "addedBlocks": ["x" * 40, "y" * 2]}},
        {"type": "attachment", "attachment": {"type": "prompt_snapshot", "systemPrompt": ["s" * 7]}},
        {"type": "attachment", "attachment": {"type": "prompt_snapshot", "systemPrompt": ["s" * 7, "p" * 3],
                                              "tools": [{"name": "Write"}, {"name": "Bash"}]}},
        {"type": "user", "timestamp": "2026-01-01T00:00:00.000Z",
         "message": {"content": [{"type": "text", "text": "Repair it."}]}},
        _assistant("m1", "2026-01-01T00:00:01.000Z", _usage(1000, 200, 50, thinking=10),
                   _tool("Bash", "t1", command="cat CONTRACT.md")),
        _result("t1", "contract text", "2026-01-01T00:00:02.000Z"),
        _assistant("m2", "2026-01-01T00:00:03.000Z", plan, {"type": "thinking", "thinking": ""}),
        _assistant("m2", "2026-01-01T00:00:09.000Z", plan, _tool("Write", "t2", file_path="a.py", content="x")),
        _result("t2", "written", "2026-01-01T00:00:10.000Z"),
        _assistant("side", "2026-01-01T00:00:10.500Z", _usage(9, 9, 9, thinking=9), isSidechain=True),
        _assistant("m3", "2026-01-01T00:00:11.000Z", _usage(1700, 50, 120, thinking=0),
                   _tool("Bash", "t3", command="python -m pytest -q")),
        _result("t3", "===== 31 passed in 0.2s =====", "2026-01-01T00:00:20.000Z"),
        _assistant("m4", "2026-01-01T00:00:21.000Z", _usage(1800, 20, 60, thinking=5),
                   _tool("Edit", "t4", file_path="a.py", old_string="x", new_string="y")),
        _result("t4", "edited", "2026-01-01T00:00:22.000Z"),
        _assistant("m5", "2026-01-01T00:00:30.500Z", _usage(1900, 10, 40, thinking=0),
                   {"type": "text", "text": "Done. I added 12 new tests and all pass."}),
    ]


def _codex(kind, stamp, **payload):
    return {"type": kind, "timestamp": stamp, "payload": payload}


def _count(stamp, total, cached, output, reasoning):
    usage = {"input_tokens": total, "cached_input_tokens": cached, "cache_write_input_tokens": 0,
             "output_tokens": output, "reasoning_output_tokens": reasoning}
    return _codex("event_msg", stamp, type="token_count", info={"last_token_usage": usage})


def _item(stamp, **item):
    return _codex("event_msg", stamp, type="item_completed", item=item)


def codex_rows(session_cwd="/elsewhere", turn_cwd=WORKSPACE):
    """Three requests: a shell read, a file change after some reasoning, a test run and the answer."""
    return [
        _codex("session_meta", "2026-01-01T00:00:00.000Z", cwd=session_cwd),
        _codex("turn_context", "2026-01-01T00:00:00.100Z", cwd=turn_cwd),
        _codex("response_item", "2026-01-01T00:00:00.200Z", type="message", role="developer",
               content=[{"type": "input_text", "text": "<permissions instructions>\n" + "p" * 10}]),
        _codex("response_item", "2026-01-01T00:00:00.300Z", type="message", role="user",
               content=[{"type": "input_text", "text": "Repair it."}]),
        _item("2026-01-01T00:00:01.000Z", type="CommandExecution", command=["zsh", "-lc", "cat CONTRACT.md"],
              aggregated_output="contract"),
        _count("2026-01-01T00:00:01.500Z", 16000, 7000, 100, 0),
        _item("2026-01-01T00:00:03.000Z", type="Reasoning"),
        _item("2026-01-01T00:00:04.000Z", type="FileChange", changes={}),
        _count("2026-01-01T00:00:04.500Z", 17000, 16000, 400, 250),
        _item("2026-01-01T00:00:06.000Z", type="CommandExecution",
              command=["zsh", "-lc", "python -m unittest"], aggregated_output="Ran 4 tests in 0.01s\n\nOK"),
        _count("2026-01-01T00:00:06.500Z", 18000, 17000, 80, 30),
        _item("2026-01-01T00:00:08.000Z", type="AgentMessage", content=[{"type": "Text", "text": "Fixed."}]),
        _codex("event_msg", "2026-01-01T00:00:08.250Z", type="token_count", info=None),
    ]


def _write_jsonl(path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n")
    return path


class ClaudeFigureTests(unittest.TestCase):
    def test_every_figure_of_the_synthetic_session(self):
        figures = transcript.claude_figures(claude_rows())
        self.assertEqual(figures["requests"], 5)
        self.assertEqual(figures["first_request_input_tokens"],
                         {"uncached": 2, "cache_read": 1000, "cache_write": 200, "total": 1202})
        self.assertEqual(figures["request_input_tokens"], [1202, 1602, 1752, 1822, 1912])
        self.assertEqual((figures["uncached_input_tokens"], figures["cache_read_tokens"],
                          figures["cache_write_tokens"]), (10, 7900, 380))
        self.assertEqual(figures["output_tokens"], 1170)
        self.assertEqual(figures["thinking_tokens_total"], 715)
        self.assertEqual(figures["first_edit_request"], 2)
        self.assertEqual(figures["thinking_tokens_before_first_edit"], 710)
        self.assertEqual(figures["tool_calls"], {"Bash": 2, "Edit": 1, "Write": 1})
        self.assertEqual((figures["test_runs"], figures["last_test_run_tests"]), (1, 31))
        self.assertEqual(figures["tests_stated_added"], 12)
        self.assertEqual(figures["seconds_from_last_change_to_end"], 9.5)

    def test_before_the_first_edit_includes_the_response_that_edits_for_both_clis(self):
        claude = transcript.claude_figures(claude_rows())
        self.assertEqual(claude["first_edit_request"], 2)
        self.assertEqual(claude["thinking_tokens_before_first_edit"], 10 + 700, "the reading and the editing response")
        codex = transcript.codex_figures(codex_rows())
        self.assertEqual(codex["first_edit_request"], 2)
        self.assertEqual(codex["thinking_tokens_before_first_edit"], 0 + 250, "the reading and the editing response")
        for module_text in (transcript.__doc__, transcript.claude_figures.__doc__, transcript.codex_figures.__doc__):
            self.assertIn("first", module_text)

    def test_what_the_first_turn_was_given(self):
        figures = transcript.claude_figures(claude_rows())
        self.assertEqual(figures["always_loaded_tools"], ["Bash", "Write"])
        self.assertEqual(figures["always_loaded_tool_chars"], len('{"name": "Write"}') + len('{"name": "Bash"}'))
        self.assertEqual(figures["system_prompt_chars"], 10)
        self.assertEqual((figures["deferred_tools"], figures["mcp_instruction_chars"]), (3, 42))
        self.assertEqual(figures["first_user_chars"], len("Repair it."))

    def test_tools_the_transcript_does_not_list_stay_unknown(self):
        rows = [row for row in claude_rows() if (row.get("attachment") or {}).get("type") != "prompt_snapshot"]
        figures = transcript.claude_figures(rows)
        self.assertIsNone(figures["always_loaded_tools"])
        self.assertIsNone(figures["always_loaded_tool_chars"])
        self.assertIsNone(figures["system_prompt_chars"])

    def test_the_largest_thinking_response_is_named(self):
        figures = transcript.claude_figures(claude_rows())
        self.assertEqual((figures["largest_thinking_request"], figures["largest_thinking_tokens"]), (2, 700))
        unrecorded = claude_rows()
        del unrecorded[-1]["message"]["usage"]["output_tokens_details"]
        figures = transcript.claude_figures(unrecorded)
        self.assertEqual((figures["largest_thinking_request"], figures["largest_thinking_tokens"]), (None, None))

    def test_a_response_split_over_rows_is_one_request_counted_once(self):
        requests = transcript.claude_requests(claude_rows())
        self.assertEqual(len(requests), 5)
        self.assertEqual([block.get("type") for block, _ in requests[1]["blocks"]], ["thinking", "tool_use"])
        self.assertEqual(requests[1]["usage"]["output_tokens"], 900)

    def test_unrecorded_thinking_stays_unknown(self):
        rows = claude_rows()
        del rows[5]["message"]["usage"]["output_tokens_details"]
        figures = transcript.claude_figures(rows)
        self.assertIsNone(figures["thinking_tokens_total"])
        self.assertIsNone(figures["thinking_tokens_before_first_edit"])
        self.assertEqual(figures["output_tokens"], 1170)

    def test_a_session_without_an_edit_has_no_first_edit_figures(self):
        rows = [row for row in claude_rows() if (row.get("message") or {}).get("id") not in ("m2", "m4")]
        figures = transcript.claude_figures(rows)
        self.assertIsNone(figures["first_edit_request"])
        self.assertIsNone(figures["thinking_tokens_before_first_edit"])
        self.assertIsNone(figures["seconds_from_last_change_to_end"])
        self.assertEqual(figures["thinking_tokens_total"], 10)

    def test_a_transcript_without_a_response_is_an_error(self):
        with self.assertRaisesRegex(transcript.TranscriptError, "no assistant response"):
            transcript.claude_figures(claude_rows()[:3])

    def test_stated_tests_are_read_only_when_stated(self):
        for text, expected in (("Added 7 tests.", 7), ("wrote 23 additional tests", 23),
                               ("5 tests added", 5), ("adds 9 focused unit tests", 9),
                               ("All 40 tests pass.", None), ("", None), (None, None)):
            self.assertEqual(transcript._first_number(transcript.TESTS_STATED, text), expected, text)


class CodexFigureTests(unittest.TestCase):
    def test_every_figure_of_the_synthetic_rollout(self):
        figures = transcript.codex_figures(codex_rows())
        self.assertEqual(figures["requests"], 3)
        self.assertEqual(figures["first_request_input_tokens"],
                         {"uncached": 9000, "cache_read": 7000, "cache_write": 0, "total": 16000})
        self.assertEqual(figures["request_input_tokens"], [16000, 17000, 18000])
        self.assertEqual((figures["uncached_input_tokens"], figures["cache_read_tokens"]), (11000, 40000))
        self.assertEqual((figures["output_tokens"], figures["thinking_tokens_total"]), (580, 280))
        self.assertEqual(figures["first_edit_request"], 2)
        self.assertEqual(figures["thinking_tokens_before_first_edit"], 250)
        self.assertEqual(figures["tool_calls"], {"CommandExecution": 2, "FileChange": 1})
        self.assertEqual((figures["test_runs"], figures["last_test_run_tests"]), (1, 4))
        self.assertIsNone(figures["tests_stated_added"])
        self.assertEqual(figures["seconds_from_last_change_to_end"], 4.25)
        self.assertIsNone(figures["always_loaded_tools"], "a rollout does not list the tools it was given")
        self.assertEqual((figures["largest_thinking_request"], figures["largest_thinking_tokens"]), (2, 250))
        self.assertEqual(figures["first_turn_context_chars"],
                         {"developer: <permissions instructions>": 37, "user: Repair it.": 10})

    def test_a_repeated_token_count_is_not_a_second_request(self):
        rows = codex_rows()
        running = 0
        for row in rows:
            info = row["payload"].get("info")
            if info:
                running += info["last_token_usage"]["input_tokens"] + info["last_token_usage"]["output_tokens"]
                info["total_token_usage"] = {"total_tokens": running}
        repeated = [*rows[:7], json.loads(json.dumps(rows[6])), *rows[7:]]
        self.assertEqual(transcript.codex_figures(repeated), transcript.codex_figures(rows))
        self.assertEqual(transcript.codex_figures(repeated)["requests"], 3)

    def test_a_rollout_without_token_counts_is_an_error(self):
        rows = [row for row in codex_rows() if row["payload"].get("type") != "token_count"]
        with self.assertRaisesRegex(transcript.TranscriptError, "no token_count event"):
            transcript.codex_figures(rows)


class FindingTranscriptTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.tmp = Path(temporary.name)
        self.claude_home, self.codex_home = self.tmp / "claude", self.tmp / "codex"
        self.project = self.claude_home / "projects" / "-work-tree-qi-1-target-hb-abc123-workspace"
        self.trial = {"condition": "native-haiku-low", "case": "matchbook", "run": 1, "short_id": "abc123",
                      "trial_dir": "/work/tree/qi_1/target/hb/abc123",
                      "requested_treatment": {"kind": "native-claude", "model": "claude-haiku-5-5"}}

    def _manifest(self, *trials):
        path = self.tmp / "manifest.json"
        path.write_text(json.dumps({"seed": 3, "trials": list(trials)}))
        return path

    def test_a_claude_session_is_found_by_the_workspace_path(self):
        saved = _write_jsonl(self.project / "session.jsonl", claude_rows())
        self.assertEqual(transcript.find_claude_transcript(WORKSPACE, self.claude_home), saved)
        row = transcript.trial_figures(self.trial, self.claude_home, {})
        self.assertEqual((row["condition"], row["short_id"], row["kind"], row["requests"]),
                         ("native-haiku-low", "abc123", "claude", 5))

    def test_a_missing_or_ambiguous_claude_session_is_reported_not_guessed(self):
        row = transcript.trial_figures(self.trial, self.claude_home, {})
        self.assertIn("found 0", row["transcript_error"])
        self.assertNotIn("requests", row)
        for name in ("one.jsonl", "two.jsonl"):
            _write_jsonl(self.project / name, claude_rows())
        self.assertIn("found 2", transcript.trial_figures(self.trial, self.claude_home, {})["transcript_error"])

    def test_a_codex_session_is_found_by_its_first_turn_directory(self):
        day = self.codex_home / "sessions" / "2026" / "01" / "01"
        saved = _write_jsonl(day / "rollout-a.jsonl", codex_rows())
        _write_jsonl(day / "rollout-b.jsonl", codex_rows(session_cwd="/other", turn_cwd=None))
        index = transcript.codex_sessions_by_workspace(self.codex_home)
        self.assertEqual(index, {WORKSPACE: [saved], "/other": [day / "rollout-b.jsonl"]})
        for treatment in ({"kind": "native-codex", "model": "gpt-6-luna"},
                          {"kind": "aether-single", "model": "codex/gpt-6-luna"}):
            row = transcript.trial_figures({**self.trial, "requested_treatment": treatment}, self.claude_home, index)
            self.assertEqual((row["kind"], row["requests"]), ("codex", 3))
        missing = transcript.trial_figures({**self.trial, "requested_treatment": {"kind": "native-codex"}},
                                           self.claude_home, {})
        self.assertIn("expected one Codex session", missing["transcript_error"])

    def test_main_reports_every_trial_of_a_manifest(self):
        _write_jsonl(self.project / "session.jsonl", claude_rows())
        unread = {**self.trial, "short_id": "zzz", "trial_dir": "/work/tree/qi_1/target/hb/zzz"}
        output = self.tmp / "figures.json"
        report = transcript.main([str(self._manifest(self.trial, unread)), "--output", str(output),
                                  "--claude-home", str(self.claude_home), "--codex-home", str(self.codex_home)])
        self.assertEqual(json.loads(output.read_text()), report)
        self.assertEqual([row["seed"] for row in report], [3, 3])
        self.assertEqual(report[0]["thinking_tokens_before_first_edit"], 710)
        self.assertIn("transcript_error", report[1])

    def test_main_refuses_a_file_that_is_not_a_manifest(self):
        path = self.tmp / "other.json"
        path.write_text(json.dumps({"trials": None}))
        with self.assertRaisesRegex(ValueError, "not a harness-bench manifest"):
            transcript.main([str(path), "--claude-home", str(self.claude_home), "--codex-home", str(self.codex_home)])


if __name__ == "__main__":
    unittest.main()
