"""Offline checks for the live wait observer's failure criteria."""
import unittest

from orchestration_wait import assess, parent_counters, reconcile_claude_agent


def event(seq, kind, agent="parent"):
    payload = {"type": kind}
    if kind == "tool.requested":
        payload["call"] = {"id": f"call-{seq}", "name": "Bash"}
    return {"seq": seq, "agent_id": agent, "kind": payload}


class WaitAssessmentTests(unittest.TestCase):
    def test_successful_wakeup_does_not_hide_a_failed_wait_result(self):
        events = [
            {"seq": 0, "agent_id": "parent", "kind": {"type": "tool.requested",
             "call": {"id": "wait", "name": "mcp__aether__wait_for"}}},
            {"seq": 1, "agent_id": "parent", "kind": {"type": "tool.succeeded",
             "call": "wait", "outcome": {"is_error": True}}},
            event(11, "agent.started"),
        ]
        self.assertIn("delegation or wait was reported as a tool error",
                      assess(events, "parent", 1, 10, {}, {}))
        events[1]["kind"]["outcome"]["is_error"] = False
        self.assertEqual(assess(events, "parent", 1, 10, {}, {}), [])

    def test_receipts_compare_only_the_claude_agent_not_the_mixed_tree(self):
        tree = {"nodes": {
            "parent": {"agent": {"id": "parent", "model": {"provider": "codex"},
                                  "stats": {"usage": {"cost_usd": 100}}}},
            "child": {"agent": {"id": "child", "model": {"provider": "claude-code"},
                                 "stats": {"usage": {"cost_usd": 0.5, "cost_complete": True}}}}}}
        captures = [{"capture_id": "one", "started_ns": 1, "resumed": False,
                     "receipt": {"type": "result", "subtype": "success",
                                 "session_id": "s", "total_cost_usd": 0.5}}]
        result = reconcile_claude_agent(tree, captures)
        self.assertEqual(result["native_minus_durable"]["api_equivalent_cost_usd"], 0)
        self.assertEqual(result["root_usage"]["api_equivalent_cost_usd"], 0.5)
        self.assertEqual(result["unattributed_processes"], 0)

    def test_child_progress_and_one_parent_resumption_are_allowed(self):
        events = [event(2, "agent.started", "child"), event(4, "agent.usage_updated", "child"),
                  event(5, "agent.turn_completed"), event(11, "agent.started")]
        self.assertEqual(assess(events, "parent", 1, 10, {}, {}), [])

    def test_every_parent_model_work_signal_while_parked_fails(self):
        for kind in ("agent.started", "snapshot.created", "agent.usage_updated", "tool.requested"):
            with self.subTest(kind=kind):
                errors = assess([event(2, kind), event(11, "agent.started")], "parent", 1, 10, {}, {})
                self.assertTrue(any("unresolved wait" in error for error in errors))

    def test_missing_or_duplicate_resume_fails(self):
        for events in ([], [event(11, "agent.started"), event(12, "agent.started")]):
            self.assertTrue(assess(events, "parent", 1, 10, {}, {}))

    def test_changed_counters_fail_even_without_a_captured_usage_event(self):
        self.assertTrue(assess([event(11, "agent.started")], "parent", 1, 10,
                               {"usage": {"input_tokens": 1}}, {"usage": {"input_tokens": 2}}))

    def test_counter_selection_excludes_unrelated_state_but_keeps_usage(self):
        stats = {"turns": 1, "usage": {"input_tokens": 12}, "tool_calls": 2, "context_tokens": 99}
        self.assertEqual(parent_counters({"stats": stats}),
                         {"turns": 1, "usage": {"input_tokens": 12}, "tool_calls": 2})


if __name__ == "__main__":
    unittest.main()
