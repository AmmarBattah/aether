"""Resumed receipts must not count previous session work again."""
import unittest

from harness_bench_usage import FIELDS, UsageError, reconcile_claude_receipts


def capture(identity, started, resumed, cost, tokens, session="s"):
    return {"capture_id": identity, "started_ns": started, "resumed": resumed,
            "agent_id": "root", "receipt": {"type": "result", "subtype": "success",
            "session_id": session, "total_cost_usd": cost,
            "modelUsage": {"main": {"inputTokens": tokens, "outputTokens": 2,
                "cacheReadInputTokens": 0, "cacheCreationInputTokens": 0}},
            "usage": {"input_tokens": 3, "output_tokens": 1,
                "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}}}


def reconcile(captures):
    return reconcile_claude_receipts({"usage": dict.fromkeys(FIELDS, 0),
        "root_agent_id": "root", "agent_ids": ["root"]}, captures)


class SessionReceiptTests(unittest.TestCase):
    def test_resume_counts_cumulative_session_once_in_timestamp_order(self):
        first = capture("999", 1, False, 0.0595702, 10)
        second = capture("111", 2, True, 0.0712778, 20)
        result = reconcile([second, first])
        self.assertEqual(result["full_process_usage"]["total_tokens"], 22)
        self.assertAlmostEqual(result["full_process_usage"]["api_equivalent_cost_usd"], 0.0712778)
        self.assertAlmostEqual(result["units"][1]["usage"]["api_equivalent_cost_usd"], 0.0117076)
        self.assertTrue(result["coverage"]["total_tokens"]["complete"])
        self.assertEqual(result["units"][1]["usage"]["output_tokens"], 0)

    def test_unknown_prior_session_uses_invocation_tokens_not_old_cost(self):
        result = reconcile([capture("a", 1, True, 10, 10000)])
        self.assertEqual(result["full_process_usage"]["total_tokens"], 4)
        self.assertIsNone(result["full_process_usage"]["api_equivalent_cost_usd"])
        self.assertFalse(result["coverage"]["api_equivalent_cost_usd"]["complete"])

    def test_unknown_baseline_can_seed_next_invocation(self):
        result = reconcile([capture("a", 1, True, 10, 10000),
                            capture("b", 2, True, 11, 10005)])
        self.assertEqual(result["full_process_usage"]["total_tokens"], 9)
        self.assertEqual(result["full_process_usage"]["api_equivalent_cost_usd"], 1)
        self.assertFalse(result["coverage"]["api_equivalent_cost_usd"]["complete"])

    def test_distinct_fresh_sessions_are_additive(self):
        result = reconcile([capture("a", 1, False, 1, 10),
                            capture("b", 2, False, 2, 20, session="other")])
        self.assertEqual(result["full_process_usage"]["total_tokens"], 34)

    def test_legacy_repeated_session_requires_order_provenance(self):
        first, second = capture("a", 1, False, 1, 10), capture("b", 2, True, 2, 20)
        del first["started_ns"]
        with self.assertRaisesRegex(UsageError, "recorder timestamps"):
            reconcile([first, second])

    def test_invalid_session_history_fails_visibly(self):
        for patch, expected in [({"started_ns": 1}, "ambiguous"),
                                ({"resumed": False}, "fresh Claude"),
                                ({"agent_id": "other"}, "attribution")]:
            with self.subTest(patch=patch):
                second = capture("b", 2, True, 2, 20)
                second.update(patch)
                with self.assertRaisesRegex(UsageError, expected):
                    reconcile([capture("a", 1, False, 1, 10), second])
        with self.assertRaisesRegex(UsageError, "decreased"):
            reconcile([capture("a", 1, False, 2, 10), capture("b", 2, True, 1, 20)])

    def test_missing_process_is_not_a_zero_usage_receipt(self):
        result = reconcile([capture("a", 1, False, 1, 10),
                            {"capture_id": "missing", "receipt": None},
                            capture("b", 3, True, 2, 20)])
        self.assertEqual(result["full_process_usage"]["total_tokens"], 22)
        self.assertFalse(result["coverage"]["total_tokens"]["complete"])

    def test_invocation_usage_is_not_subtracted_when_model_usage_is_absent(self):
        first, second = capture("a", 1, False, 1, 10), capture("b", 2, True, 2, 20)
        for row in (first, second):
            del row["receipt"]["modelUsage"]
        result = reconcile([first, second])
        self.assertEqual(result["full_process_usage"]["total_tokens"], 8)
        self.assertEqual(result["full_process_usage"]["api_equivalent_cost_usd"], 2)

    def test_missing_cumulative_token_baseline_stays_unknown(self):
        first, second = capture("a", 1, False, 1, 10), capture("b", 2, True, 2, 20)
        del first["receipt"]["modelUsage"]
        result = reconcile([first, second])
        self.assertIsNone(result["units"][1]["usage"]["total_tokens"])
        self.assertEqual(result["full_process_usage"]["api_equivalent_cost_usd"], 2)
        self.assertFalse(result["coverage"]["total_tokens"]["complete"])

    def test_malformed_receipt_fails_with_accounting_error(self):
        for receipt in [[], {"session_id": []}, {"session_id": ""}]:
            with self.subTest(receipt=receipt), self.assertRaises(UsageError):
                reconcile([{"capture_id": "one", "receipt": receipt}])


if __name__ == "__main__":
    unittest.main()
