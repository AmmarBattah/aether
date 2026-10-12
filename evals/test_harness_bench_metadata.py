"""Offline regressions for matched native/Aether configuration evidence."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock

import harness_bench as hb
from harness_bench_metadata import aether_resolved_metadata, codex_session_metadata


class MatchedTreatmentTests(unittest.TestCase):
    def test_default_matrix_is_only_the_four_single_agent_paths(self):
        self.assertEqual(hb.parse_args([]).conditions.split(","), [
            "native-claude", "native-codex", "aether-single-claude", "aether-single-codex",
        ])
        self.assertEqual(hb._resolve_conditions("aether-coordinator-team"), ["aether-coordinator-team"])

    def test_single_agent_preserves_native_effort_and_uses_neutral_role(self):
        for key in ("aether-single-claude", "aether-single-codex"):
            with self.subTest(key=key):
                client = Mock()
                client.create_agent.return_value = {"id": "root"}
                self.assertEqual(hb._spawn_root(client, hb.CONDITIONS[key], "task"), ("root", None))
                client.create_agent.assert_called_once_with(
                    role="assistant", objective="task", model=hb.CONDITIONS[key]["model"], long_lived=False)


class ResolvedMetadataTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.env = {"CODEX_HOME": str(self.root)}
        sessions = self.root / "sessions/2026/09/21"
        sessions.mkdir(parents=True)
        self.session = "123456-abcd"
        self.path = sessions / f"rollout-{self.session}.jsonl"
        self.path.write_text("\n".join([
            "not json", "null", "[]",
            json.dumps({"type": "turn_context", "payload": None}),
            json.dumps({"type": "turn_context", "payload": {
                "model": "test-model", "effort": "high", "instructions": "SECRET"}}),
        ]))

    def test_native_and_aether_capture_the_same_effective_settings(self):
        native = hb.native_resolved_metadata(json.dumps({
            "type": "thread.started", "thread_id": self.session}), self.env, False)
        events = [{"agent_id": "root", "kind": {"type": "custom", "name": "engine.session",
                   "payload": {"session": self.session, "model": "test-model", "tools": ["SECRET"]}}}]
        agents = [{"id": "root", "model": {"provider": "codex"}, "config": {"effort": "low"}}]
        aether = aether_resolved_metadata(events, agents, self.env)
        self.assertEqual(aether[1], {**native[0], "agent_id": "root"})
        self.assertEqual(aether[1]["effort"], "high", "actual session beats requested effort")
        self.assertNotIn("SECRET", json.dumps(aether))

    def test_missing_session_is_unknown_not_an_inferred_default(self):
        self.assertEqual(codex_session_metadata(["abcdef"], self.env), [])

    def test_invalid_session_ids_cannot_broaden_the_scan(self):
        self.assertEqual(codex_session_metadata([None, 42, [], "", "*", "../"], self.env), [])
        self.assertEqual(len(codex_session_metadata([None, self.session, self.session], self.env)), 1)

    def test_claude_does_not_read_codex_session_or_infer_effort(self):
        events = [{"agent_id": "root", "kind": {"type": "custom", "name": "engine.session",
                   "payload": {"session": self.session, "model": "claude-test"}}}]
        agents = [{"id": "root", "model": {"provider": "claude-code"}, "config": {"effort": "high"}}]
        metadata = aether_resolved_metadata(events, agents, self.env)
        self.assertEqual(len(metadata), 1)
        self.assertIsNone(metadata[0]["effort"])


if __name__ == "__main__":
    unittest.main()
