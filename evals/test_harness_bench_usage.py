"""Synthetic accounting checks only: no daemon, network, paid CLI, or scratch files."""

import json
import unittest

from harness_bench_usage import (
    UsageError,
    normalize_claude_receipt,
    normalize_usage,
    parse_aether_events,
    parse_claude_jsonl,
    parse_codex_jsonl,
    reconcile_claude_receipts,
)


def jsonl(*events):
    return "".join(json.dumps(event) + "\n" for event in events)


def usage(inp=10, out=5, read=20, write=3, **extra):
    return {"input_tokens": inp, "output_tokens": out, "cache_read_tokens": read,
            "cache_write_tokens": write, "cost_usd": 0, "cost_complete": False, **extra}


def model_usage(inp=10, out=5, read=20, write=3, **extra):
    return {"inputTokens": inp, "outputTokens": out, "cacheReadInputTokens": read,
            "cacheCreationInputTokens": write, **extra}


def event(seq, agent, typ, **fields):
    return {"seq": seq, "id": f"event_{seq}", "agent_id": agent, "kind": {"type": typ, **fields}}


class NormalizationTests(unittest.TestCase):
    def test_codex_cache_and_reasoning_are_subsets(self):
        result = normalize_usage({"input_tokens": 100, "cached_input_tokens": 80,
                                  "output_tokens": 30, "reasoning_output_tokens": 20}, "codex")
        self.assertEqual(result["total_tokens"], 130)
        self.assertEqual(result["uncached_input_tokens"], 20)
        self.assertIsNone(result["cache_write_tokens"])

    def test_claude_does_not_add_cache_breakdowns_iterations_or_thinking(self):
        raw = {"input_tokens": 10, "output_tokens": 9, "cache_read_input_tokens": 100,
               "cache_creation_input_tokens": 20, "output_tokens_details": {"thinking_tokens": 7},
               "cache_creation": {"ephemeral_5m_input_tokens": 20},
               "iterations": [{"input_tokens": 1000}]}
        self.assertEqual(normalize_usage(raw, "claude")["total_tokens"], 139)

    def test_missing_fields_are_not_zero(self):
        result = normalize_usage({"input_tokens": 10, "output_tokens": 5}, "claude")
        self.assertEqual(result["uncached_input_tokens"], 10)
        self.assertIsNone(result["input_tokens"])
        self.assertIsNone(result["total_tokens"])
        self.assertTrue(all(value is None for value in normalize_usage(None, "aether").values()))

    def test_zero_is_reported_and_unpriced_zero_is_unknown(self):
        result = normalize_usage(usage(0, 0, 0, 0), "aether")
        self.assertEqual(result["total_tokens"], 0)
        self.assertIsNone(result["api_equivalent_cost_usd"])
        result = normalize_usage(usage(0, 0, 0, 0, cost_complete=True), "aether")
        self.assertEqual(result["api_equivalent_cost_usd"], 0)

    def test_invalid_counters_fail_visibly(self):
        for value in (-1, True, 1.5, "5", float("nan"), float("inf")):
            with self.subTest(value=value), self.assertRaises(UsageError):
                normalize_usage({"input_tokens": value}, "codex")
        with self.assertRaises(UsageError):
            normalize_usage({"input_tokens": 5, "cached_input_tokens": 6}, "codex")
        with self.assertRaises(UsageError):
            normalize_usage({"output_tokens": 5, "reasoning_output_tokens": 6}, "codex")


class CodexTests(unittest.TestCase):
    def test_counts_terminal_usage_not_cumulative_samples(self):
        report = parse_codex_jsonl(jsonl(
            {"type": "thread.started", "thread_id": "thread"}, {"type": "turn.started"},
            {"type": "token_count", "usage": {"input_tokens": 100000}},
            {"type": "turn.completed", "usage": {"input_tokens": 100, "cached_input_tokens": 60, "output_tokens": 20}},
            {"type": "turn.started"},
            {"type": "turn.completed", "usage": {"input_tokens": 50, "output_tokens": 10}},
        ))
        self.assertEqual(report["usage"]["total_tokens"], 180)
        self.assertEqual(report["usage"]["cache_read_tokens"], 60)
        self.assertEqual(report["coverage"]["cache_read_tokens"]["reported"], 1)
        self.assertFalse(report["coverage"]["cache_read_tokens"]["complete"])

    def test_failed_retry_keeps_missing_coverage(self):
        report = parse_codex_jsonl(jsonl(
            {"type": "turn.started"}, {"type": "error", "message": "not retained"},
            {"type": "turn.failed"}, {"type": "turn.started"},
            {"type": "turn.completed", "usage": {"input_tokens": 5, "output_tokens": 2}},
        ))
        self.assertEqual(report["usage"]["total_tokens"], 7)
        self.assertEqual(report["coverage"]["total_tokens"]["units"], 2)
        self.assertFalse(report["coverage"]["total_tokens"]["complete"])
        self.assertNotIn("not retained", json.dumps(report))

    def test_duplicate_terminal_and_duplicate_thread_rejected(self):
        for events in [
            [{"type": "turn.completed"}, {"type": "turn.completed"}],
            [{"type": "thread.started", "thread_id": "same"}] * 2,
        ]:
            with self.subTest(events=events), self.assertRaises(UsageError):
                parse_codex_jsonl(jsonl(*events))

    def test_partial_tail_and_malformed_complete_line(self):
        report = parse_codex_jsonl('{"type":"turn.started"}\n{"unfinished"')
        self.assertIsNone(report["usage"]["total_tokens"])
        self.assertIn("incomplete trailing", " ".join(report["warnings"]))
        with self.assertRaises(UsageError):
            parse_codex_jsonl('{not json}\n')
        with self.assertRaises(UsageError):
            parse_codex_jsonl('[]\n')

    def test_tool_lifecycle_is_not_multiple_calls(self):
        item = {"id": "tool1", "type": "command_execution", "command": "SECRET"}
        report = parse_codex_jsonl(jsonl({"type": "item.started", "item": item},
                                        {"type": "item.completed", "item": item}))
        self.assertEqual(report["tool_calls"], {"command_execution": 1})
        self.assertNotIn("SECRET", json.dumps(report))


class ClaudeTests(unittest.TestCase):
    def test_models_include_helper_and_replace_primary_usage(self):
        receipt = {"type": "result", "subtype": "success", "total_cost_usd": 0.4,
                   "usage": {"input_tokens": 999999},
                   "modelUsage": {"primary": model_usage(), "helper": model_usage(2, 1, 0, 0)}}
        report = normalize_claude_receipt(receipt)
        self.assertEqual(report["usage"]["total_tokens"], 41)
        self.assertEqual(report["usage"]["api_equivalent_cost_usd"], 0.4)
        self.assertIn("API-equivalent", report["cost_basis"])
        self.assertEqual(len(report["units"][0]["models"]), 2)

    def test_missing_model_counter_is_not_zero(self):
        receipt = {"type": "result", "subtype": "success", "modelUsage": {
            "one": model_usage(thinkingTokens=2), "two": model_usage()}}
        report = normalize_claude_receipt(receipt)
        self.assertIsNone(report["usage"]["reasoning_tokens"])
        self.assertEqual(report["units"][0]["model_coverage"]["reasoning_tokens"]["reported"], 1)

    def test_provisional_output_does_not_compete_with_terminal(self):
        report = parse_claude_jsonl(jsonl(
            {"type": "assistant", "message": {"id": "msg", "usage": {"output_tokens": 1}}},
            {"type": "assistant", "message": {"id": "msg", "usage": {"output_tokens": 1}}},
            {"type": "result", "subtype": "success", "modelUsage": {"main": model_usage(out=100)}},
        ))
        self.assertEqual(report["usage"]["output_tokens"], 100)

    def test_missing_result_and_error_result(self):
        report = parse_claude_jsonl(jsonl({"type": "assistant", "message": {"usage": {"output_tokens": 5}}}))
        self.assertIsNone(report["usage"]["output_tokens"])
        report = normalize_claude_receipt({"type": "result", "subtype": "error_max_turns",
                                           "modelUsage": {"main": model_usage()}})
        self.assertEqual(report["units"][0]["status"], "failed")
        self.assertEqual(report["usage"]["total_tokens"], 38)

    def test_multiple_prompt_results_rejected_but_notification_skipped(self):
        result = {"type": "result", "subtype": "success"}
        with self.assertRaises(UsageError):
            parse_claude_jsonl(jsonl(result, result))
        report = parse_claude_jsonl(jsonl({**result, "origin": {"kind": "task-notification"}}, result))
        self.assertEqual(len(report["units"]), 1)
        self.assertIn("notification", " ".join(report["warnings"]))

    def test_no_transcript_or_nested_metadata_retained(self):
        result = {"type": "result", "subtype": "success", "result": "SECRET",
                  "modelUsage": {"main": model_usage(secret="SECRET")}, "argv": ["SECRET"]}
        self.assertNotIn("SECRET", json.dumps(normalize_claude_receipt(result)))


class AetherTests(unittest.TestCase):
    def test_latest_cumulative_wins_after_message_placeholder_and_turn_close(self):
        report = parse_aether_events([
            event(1, "root", "agent.created", model={"provider": "codex", "model": "default"}),
            event(2, "root", "agent.started", turn=1),
            event(3, "root", "snapshot.created", snapshot="snap", turn=1, model={"provider": "codex", "model": "m"}),
            event(4, "root", "agent.usage_updated", turn_usage=usage(out=1), total=usage(out=1)),
            event(5, "root", "invocation.completed", usage=usage(0, 0, 0, 0)),
            event(6, "root", "agent.usage_updated", turn_usage=usage(out=100, reasoning_tokens=80), total=usage(out=100)),
            event(7, "root", "agent.turn_completed", turn=1, next="waiting"),
        ], "root", initial_efforts={"root": "medium"})
        self.assertEqual(report["usage"]["total_tokens"], 133)
        self.assertEqual(report["usage"]["reasoning_tokens"], 80)
        self.assertEqual(report["units"][0]["effort"], "medium")
        self.assertEqual(report["units"][0]["usage_updates"], 2)
        self.assertTrue(report["coverage"]["total_tokens"]["complete"])

    def test_descendants_failed_retry_and_unrelated_agent(self):
        events = [event(1, "root", "agent.created"), event(2, "child", "agent.created", parent="root"),
                  event(3, "grandchild", "agent.created", parent="child")]
        for agent, turn, seq in [("root", 1, 4), ("child", 1, 7), ("child", 2, 10), ("grandchild", 1, 13), ("unrelated", 1, 16)]:
            events += [event(seq, agent, "agent.started", turn=turn),
                       event(seq + 1, agent, "agent.usage_updated", turn_usage=usage()),
                       event(seq + 2, agent, "agent.turn_completed", turn=turn, next="failed")]
        report = parse_aether_events(events, "root")
        self.assertEqual(report["usage"]["total_tokens"], 4 * 38)
        self.assertEqual(report["root_usage"]["total_tokens"], 38)
        self.assertEqual(report["descendant_usage"]["total_tokens"], 3 * 38)
        self.assertEqual(report["agent_ids"], ["child", "grandchild", "root"])

    def test_snapshots_are_metadata_not_additional_usage(self):
        events = [event(1, "root", "agent.started", turn=1),
                  event(2, "root", "snapshot.created", snapshot="first", model={"model": "one"}),
                  event(3, "root", "agent.usage_updated", turn_usage=usage()),
                  event(4, "root", "invocation.failed", snapshot="first", retryable=True),
                  event(5, "root", "snapshot.created", snapshot="second", model={"model": "two"}),
                  event(6, "root", "snapshot.created", snapshot="second", model={"model": "two"}),
                  event(7, "root", "agent.usage_updated", turn_usage=usage(out=50)),
                  event(8, "root", "agent.turn_completed", turn=1, next="completed")]
        report = parse_aether_events(events, "root")
        self.assertEqual(report["usage"]["total_tokens"], 83)
        self.assertEqual(len(report["units"][0]["attempts"]), 2)
        self.assertEqual(report["units"][0]["attempts"][0]["status"], "failed")
        self.assertIsNone(report["units"][0]["attempts"][0]["usage"])

    def test_terminal_field_missing_does_not_reuse_provisional_optional(self):
        events = [event(1, "root", "agent.started", turn=1),
                  event(2, "root", "agent.usage_updated", turn_usage=usage(reasoning_tokens=2)),
                  event(3, "root", "agent.usage_updated", turn_usage=usage(out=10)),
                  event(4, "root", "agent.turn_completed", turn=1, next="completed")]
        report = parse_aether_events(events, "root")
        self.assertIsNone(report["usage"]["reasoning_tokens"])
        self.assertIn("dropped reasoning_tokens", " ".join(report["warnings"]))

    def test_open_turn_usage_is_provisional(self):
        report = parse_aether_events([event(1, "root", "agent.started", turn=1),
                                     event(2, "root", "agent.usage_updated", turn_usage=usage())], "root")
        self.assertEqual(report["usage"]["total_tokens"], 38)
        self.assertFalse(report["coverage"]["total_tokens"]["complete"])

    def test_completed_placeholder_is_not_known_zero_usage(self):
        report = parse_aether_events([event(1, "root", "agent.started", turn=1),
                                     event(2, "root", "invocation.completed", usage=usage(0, 0, 0, 0)),
                                     event(3, "root", "agent.turn_completed", turn=1)], "root")
        self.assertIsNone(report["usage"]["total_tokens"])

    def test_duplicate_subscription_replay_dedupes_and_conflicts_fail(self):
        started = event(1, "root", "agent.started", turn=1)
        counted = event(2, "root", "agent.usage_updated", turn_usage=usage())
        report = parse_aether_events([counted, started, counted, started], "root")
        self.assertEqual(report["usage"]["total_tokens"], 38)
        with self.assertRaises(UsageError):
            parse_aether_events([started, {**started, "agent_id": "other"}], "root")

    def test_decreasing_counter_is_rejected(self):
        with self.assertRaises(UsageError):
            parse_aether_events([event(1, "root", "agent.started", turn=1),
                                 event(2, "root", "agent.usage_updated", turn_usage=usage(out=10)),
                                 event(3, "root", "agent.usage_updated", turn_usage=usage(out=1))], "root")

    def test_missing_earlier_spend_detected_by_absolute_total(self):
        with self.assertRaisesRegex(UsageError, "cumulative agent total"):
            parse_aether_events([
                event(1, "root", "agent.started", turn=2),
                event(2, "root", "agent.usage_updated", turn_usage=usage(), total=usage(out=100)),
                event(3, "root", "agent.turn_completed", turn=2),
            ], "root")

    def test_restart_root_and_tool_retries_are_included(self):
        report = parse_aether_events([
            event(1, "root", "agent.created"),
            event(2, "new", "agent.restarted", **{"from": "root", "to": "new"}),
            event(3, "new", "agent.started", turn=1),
            event(4, "new", "tool.requested", call={"id": "tool", "name": "bash", "input": "SECRET"}),
            event(5, "new", "tool.retried", call="tool", attempt=2),
            event(6, "new", "agent.usage_updated", turn_usage=usage()),
        ], "root")
        self.assertEqual(report["tool_calls"], {"bash": 1})
        self.assertEqual(report["units"][0]["tool_retries"], 1)
        self.assertNotIn("SECRET", json.dumps(report))

    def test_configuration_changes_are_applied_in_event_order(self):
        report = parse_aether_events([
            event(1, "root", "agent.started", turn=1),
            event(2, "root", "agent.turn_completed", turn=1),
            event(3, "root", "agent.config_updated", config={"effort": "high"}),
            event(4, "root", "agent.started", turn=2),
        ], "root", initial_efforts={"root": "low"})
        self.assertEqual([u["effort"] for u in report["units"]], ["low", "high"])

    def test_partial_dollar_floor_not_complete(self):
        report = parse_aether_events([
            event(1, "root", "agent.started", turn=1),
            event(2, "root", "agent.usage_updated", turn_usage=usage(cost_usd=1.25)),
            event(3, "root", "agent.turn_completed", turn=1),
        ], "root")
        self.assertEqual(report["usage"]["api_equivalent_cost_usd"], 1.25)
        self.assertFalse(report["coverage"]["api_equivalent_cost_usd"]["complete"])


class ReconciliationTests(unittest.TestCase):
    def test_sources_compared_not_added_and_missing_process_retained(self):
        durable = parse_aether_events([
            event(1, "root", "agent.started", turn=1),
            event(2, "root", "agent.usage_updated", turn_usage=usage()),
            event(3, "root", "agent.turn_completed", turn=1),
        ], "root")
        receipt = {"type": "result", "subtype": "success", "modelUsage": {
            "main": model_usage(), "helper": model_usage(2, 1, 0, 0)}}
        report = reconcile_claude_receipts(durable, [
            {"capture_id": "one", "agent_id": "root", "receipt": receipt},
            {"capture_id": "two", "receipt": None},
        ])
        self.assertEqual(report["full_process_usage"]["total_tokens"], 41)
        self.assertEqual(report["native_minus_durable"]["total_tokens"], 3)
        self.assertEqual(report["root_usage"]["total_tokens"], 41)
        self.assertFalse(report["coverage"]["total_tokens"]["complete"])
        self.assertEqual(report["unattributed_processes"], 1)
        self.assertEqual(durable["usage"]["total_tokens"], 38)

    def test_duplicate_process_receipts_fail(self):
        with self.assertRaises(UsageError):
            reconcile_claude_receipts({}, [{"capture_id": "same"}, {"capture_id": "same"}])


if __name__ == "__main__":
    unittest.main()
