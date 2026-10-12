"""Reject false positives in the native parked-restart drill."""
import unittest
from orchestration_restart import assess


class RestartAssessmentTests(unittest.TestCase):
    def setUp(self):
        self.parked = {"seq": 2, "kind": {"condition": {
            "on": "deadline", "at": "2026-09-21T00:01:00Z"}}}
        self.events = [
            {"seq": 1, "agent_id": "root", "kind": {"type": "tool.requested", "call": {
                "name": "mcp__aether__wait_for", "input": {"timeout_seconds": 60}}}},
            {"seq": 3, "agent_id": "root", "at": "2026-09-21T00:01:00Z",
             "kind": {"type": "agent.started"}},
            {"seq": 4, "agent_id": "root", "kind": {"type": "agent.message", "role": "assistant",
                "content": [{"type": "text", "text": "RESTART_OK"}]}},
        ]

    def check(self, before=1, recovered=1, after=1):
        return assess(self.events, "root", self.parked, before, recovered, after)

    def test_one_deadline_wake_passes(self):
        self.assertEqual(self.check(), [])

    def test_counter_growth_fails(self):
        self.assertTrue(self.check(recovered=2))
        self.assertTrue(self.check(after=2))

    def test_early_wake_fails(self):
        self.events[1]["at"] = "2026-09-21T00:00:59Z"
        self.assertTrue(self.check())

    def test_duplicate_wake_fails(self):
        self.events.append(self.events[1])
        self.assertTrue(self.check())

    def test_extra_tool_work_fails(self):
        self.events.append({"seq": 5, "agent_id": "root", "kind": {"type": "tool.requested",
            "call": {"name": "bash", "input": {"command": "sleep 1"}}}})
        self.assertTrue(self.check())

    def test_duplicate_wait_fails(self):
        self.events.append(self.events[0])
        self.assertTrue(self.check())

    def test_wrong_final_answer_fails(self):
        self.events[-1]["kind"]["content"][0]["text"] = "not finished"
        self.assertTrue(self.check())


if __name__ == "__main__":
    unittest.main()
