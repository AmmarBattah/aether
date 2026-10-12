"""Offline rejection checks for the live subtree cancellation observer."""
import copy
import unittest

from orchestration_cancel import assess_cancel


class CancellationAssessmentTests(unittest.TestCase):
    def setUp(self):
        self.tree = {"nodes": {
            aid: {"agent": {"status": {"state": "cancelled"}}}
            for aid in ("parent", "child")}}
        self.events = [
            {"seq": seq, "agent_id": aid, "kind": {"type": "agent.cancelled"}}
            for seq, aid in ((11, "parent"), (12, "child"))]

    def assess(self, *, events=None, tree=None, after=None):
        return assess_cancel(self.events if events is None else events, "parent", 1, 10,
                             {"turns": 1}, {"turns": 1} if after is None else after,
                             self.tree if tree is None else tree)

    def test_both_cancelled_without_parent_work_passes(self):
        self.assertEqual(self.assess(), [])

    def test_missing_child_and_extra_child_fail(self):
        for ids in (("parent",), ("parent", "child", "extra")):
            tree = {"nodes": {aid: copy.deepcopy(self.tree["nodes"]["parent"]) for aid in ids}}
            self.assertTrue(self.assess(tree=tree))

    def test_parent_missing_fails_even_with_two_nodes(self):
        tree = copy.deepcopy(self.tree)
        tree["nodes"]["other"] = tree["nodes"].pop("parent")
        self.assertTrue(self.assess(tree=tree))

    def test_running_or_completed_child_does_not_count_as_cancelled(self):
        for state in ("running", "completed", "waiting", "failed"):
            tree = copy.deepcopy(self.tree)
            tree["nodes"]["child"]["agent"]["status"]["state"] = state
            self.assertTrue(self.assess(tree=tree))

    def test_missing_duplicate_or_pre_request_cancellation_fails(self):
        for events in (self.events[:1], self.events + [self.events[1]],
                       [{**event, "seq": 9} for event in self.events]):
            self.assertTrue(self.assess(events=events))

    def test_parent_work_before_or_after_cancel_fails(self):
        for seq in (2, 13):
            for kind in ("agent.started", "snapshot.created", "agent.usage_updated", "tool.requested"):
                events = self.events + [{"seq": seq, "agent_id": "parent", "kind": {"type": kind}}]
                self.assertTrue(self.assess(events=events))

    def test_counter_change_without_event_still_fails(self):
        self.assertTrue(self.assess(after={"turns": 2}))

    def test_child_usage_drain_is_not_parent_work(self):
        events = self.events + [{"seq": 13, "agent_id": "child",
                                 "kind": {"type": "agent.usage_updated"}}]
        self.assertEqual(self.assess(events=events), [])


if __name__ == "__main__":
    unittest.main()
