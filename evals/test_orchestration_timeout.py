"""Rejection tests for deadline-specific orchestration assertions."""
import copy
import unittest

from orchestration_timeout import assess_timeout, deadlines


class TimeoutAssessmentTests(unittest.TestCase):
    def setUp(self):
        self.parked = {"seq": 2, "kind": {"condition": {"on": "any", "of": [
            {"on": "task", "id": "task"},
            {"on": "deadline", "at": "2026-09-21T00:00:40Z"}]}}}
        self.events = [
            {"seq": 1, "agent_id": "parent", "kind": {"type": "tool.requested", "call": {
                "name": "mcp__aether__wait_for", "input": {"tasks": ["task"], "timeout_seconds": 40}}}},
            {"seq": 3, "agent_id": "parent", "at": "2026-09-21T00:00:40Z", "kind": {"type": "agent.started"}},
            {"seq": 4, "agent_id": "parent", "kind": {"type": "tool.requested", "call": {
                "name": "mcp__aether__cancel_agent", "input": {"agent": "child"}}}},
            {"seq": 5, "agent_id": "child", "kind": {"type": "agent.cancelled"}},
        ]

    def assess(self, events=None):
        return assess_timeout(self.events if events is None else events,
                              "parent", "child", "task", self.parked)

    def test_deadline_resume_then_explicit_child_cancel_passes(self):
        self.assertEqual(self.assess(), [])

    def test_early_resume_fails(self):
        self.events[1]["at"] = "2026-09-21T00:00:39Z"
        self.assertTrue(self.assess())

    def test_child_terminal_before_resume_fails(self):
        for kind in ("agent.completed", "agent.failed", "agent.cancelled"):
            self.assertTrue(self.assess(self.events + [
                {"seq": 2, "agent_id": "child", "kind": {"type": kind}}]))

    def test_missing_cancel_and_duplicate_resume_fail(self):
        self.assertTrue(self.assess(self.events[:3]))
        self.assertTrue(self.assess(self.events + [copy.deepcopy(self.events[1])]))

    def test_wrong_timeout_or_repeated_wait_fails(self):
        self.assertTrue(self.assess(self.events + [copy.deepcopy(self.events[0])]))
        self.events[0]["kind"]["call"]["input"]["timeout_seconds"] = 1
        self.assertTrue(self.assess())

    def test_child_cancel_without_parent_cancel_tool_fails(self):
        self.assertTrue(self.assess(self.events[:2] + self.events[3:]))

    def test_wait_must_name_only_the_delegated_task(self):
        for tasks in ([], ["different"], ["task", "different"]):
            self.events[0]["kind"]["call"]["input"]["tasks"] = tasks
            self.assertTrue(self.assess())

    def test_cancel_must_target_the_delegated_child(self):
        self.events[2]["kind"]["call"]["input"]["agent"] = "different"
        self.assertTrue(self.assess())

    def test_missing_deadline_fails(self):
        self.parked["kind"]["condition"] = {"on": "task", "id": "task"}
        self.assertTrue(self.assess())

    def test_nested_deadline_is_found_without_matching_task_ids(self):
        self.assertEqual(deadlines({"on": "all", "of": [self.parked["kind"]["condition"]]}),
                         ["2026-09-21T00:00:40Z"])


if __name__ == "__main__":
    unittest.main()
