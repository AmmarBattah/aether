"""Focused, offline unit tests for evals/harness_bench.py.

No paid CLI, no real daemon, no network: native command assembly is checked
by argv inspection; timeout/failure recording uses cheap real subprocesses
(`true`/`sleep`); Aether wait/tree logic uses fake in-memory clients. Run with

    python3 -m unittest discover -s evals -p 'test_harness_bench*.py'
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch, Mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import harness_bench as hb

from harness_bench_test_support import TEST_SCRATCH, node


class NativeCommandAssemblyTests(unittest.TestCase):
    def test_claude_command_omits_model_and_effort_and_disables_mcp(self):
        command = hb.native_claude_command("do the task")
        self.assertEqual(command[0], "claude")
        self.assertIn("--dangerously-skip-permissions", command)
        self.assertIn("--strict-mcp-config", command)
        self.assertNotIn("--mcp-config", command)
        for flag in ("--model", "--effort"):
            self.assertNotIn(flag, command, f"{flag} would override the CLI's own installed default")
        self.assertEqual(command[-1], "do the task")

    def test_codex_command_omits_model_and_effort_and_disables_mcp(self):
        command = hb.native_codex_command("do the task")
        self.assertEqual(command[:3], ["codex", "exec", "--json"])
        self.assertIn("--dangerously-bypass-approvals-and-sandbox", command)
        self.assertIn("mcp_servers={}", command)
        for flag in ("-m", "model_reasoning_effort"):
            self.assertFalse(any(flag in part for part in command),
                              f"{flag!r} would override the CLI's own installed default")
        self.assertEqual(command[-1], "do the task")

    def test_sanitized_native_env_strips_live_aether_routing_vars(self):
        original = dict(os.environ)
        os.environ["AETHER_ADDRESS"] = "127.0.0.1:9"
        os.environ["AETHER_TOKEN"] = "shh"
        try:
            env, stripped = hb.sanitized_native_env()
        finally:
            os.environ.clear()
            os.environ.update(original)
        self.assertIn("AETHER_ADDRESS", stripped)
        self.assertIn("AETHER_TOKEN", stripped)
        self.assertNotIn("AETHER_ADDRESS", env)
        self.assertNotIn("AETHER_TOKEN", env)

    def test_strip_live_routing_env_keeps_allowlisted_keys_only(self):
        env = {"AETHER_SOCKET": "/x/sock", "AETHER_ADDRESS": "1.2.3.4:5", "PATH": "/bin"}
        removed = hb.strip_live_routing_env(env, keep={"AETHER_SOCKET"})
        self.assertEqual(removed, ["AETHER_ADDRESS"])
        self.assertEqual(env, {"AETHER_SOCKET": "/x/sock", "PATH": "/bin"})


class FakeClient:
    """Minimal stand-in for AetherClient: pre-scripted get_tree pages and a
    queue of event frames, no socket/daemon involved."""

    def __init__(self, tree_pages=None, frames=None):
        self._tree_pages = list(tree_pages or [])
        self._frames = list(frames or [])

    def request(self, command, timeout=5):
        assert command.get("cmd") == "get_tree", f"FakeClient only stubs get_tree, got {command!r}"
        assert self._tree_pages, "FakeClient.request(get_tree) called more times than scripted"
        return {"data": self._tree_pages.pop(0) if len(self._tree_pages) > 1 else self._tree_pages[0]}

    def next_frame(self, timeout):
        assert timeout > 0, "next_frame must be called with a positive timeout"
        if self._frames:
            return self._frames.pop(0)
        return {"frame": "event", "event": {"agent_id": "other", "kind": {"type": "noop"}}}


class TreeQuiescenceTests(unittest.TestCase):
    def test_all_terminal_nodes_is_quiescent(self):
        tree = {"nodes": {"root": node("root", "completed", children=["c1"]), "c1": node("c1", "failed")}}
        self.assertTrue(hb.tree_is_quiescent(tree, "root"))

    def test_root_waiting_on_inbox_with_terminal_children_is_quiescent(self):
        tree = {"nodes": {"root": node("root", "waiting", children=["c1"]), "c1": node("c1", "completed")}}
        self.assertTrue(hb.tree_is_quiescent(tree, "root"))

    def test_a_still_running_child_is_not_quiescent(self):
        tree = {"nodes": {"root": node("root", "completed", children=["c1"]), "c1": node("c1", "running")}}
        self.assertFalse(hb.tree_is_quiescent(tree, "root"))

    def test_a_non_root_agent_waiting_on_inbox_is_not_quiescent(self):
        tree = {"nodes": {"root": node("root", "completed", children=["c1"]), "c1": node("c1", "waiting")}}
        self.assertFalse(hb.tree_is_quiescent(tree, "root"))

    def test_missing_root_is_a_programmer_error(self):
        with self.assertRaises(AssertionError):
            hb.tree_is_quiescent({"nodes": {"other": node("other", "completed")}}, "root")

    def test_wait_for_tree_quiescent_returns_true_once_settled(self):
        running = {"nodes": {"root": node("root", "completed", children=["c1"]), "c1": node("c1", "running")}}
        settled = {"nodes": {"root": node("root", "completed", children=["c1"]), "c1": node("c1", "completed")}}
        client = FakeClient(tree_pages=[running, settled])
        tree, quiescent = hb.wait_for_tree_quiescent(client, run_id="run1", root_id="root", timeout=0.3)
        self.assertTrue(quiescent)
        self.assertEqual(tree, settled)

    def test_wait_for_tree_quiescent_times_out_if_never_settled(self):
        stuck = {"nodes": {"root": node("root", "completed", children=["c1"]), "c1": node("c1", "running")}}
        client = FakeClient(tree_pages=[stuck])
        _tree, quiescent = hb.wait_for_tree_quiescent(client, run_id="run1", root_id="root", timeout=0.05)
        self.assertFalse(quiescent)


class WaitForRunTerminalTests(unittest.TestCase):
    def test_completed_event_reports_completed(self):
        frames = [{"frame": "event", "event": {"agent_id": "root", "kind": {"type": "agent.completed"}}}]
        status, error, _reply = hb.wait_for_run_terminal(FakeClient(frames=frames), "root", timeout=5)
        self.assertEqual(status, "completed")
        self.assertIsNone(error)

    def test_failed_event_reports_failed_with_error_message(self):
        frames = [{"frame": "event", "event": {"agent_id": "root", "kind": {"type": "agent.failed", "error": "boom"}}}]
        status, error, _reply = hb.wait_for_run_terminal(FakeClient(frames=frames), "root", timeout=5)
        self.assertEqual(status, "failed")
        self.assertEqual(error, "boom")

    def test_inbox_wait_before_completion_is_reported_distinctly(self):
        frames = [{"frame": "event", "event": {"agent_id": "root",
                                                "kind": {"type": "agent.waiting", "condition": {"on": "inbox"}}}}]
        status, error, _reply = hb.wait_for_run_terminal(FakeClient(frames=frames), "root", timeout=5)
        self.assertEqual(status, "waiting")
        self.assertIsNotNone(error)

    def test_no_terminal_event_within_timeout_is_reported_as_timeout(self):
        status, error, reply = hb.wait_for_run_terminal(FakeClient(frames=[]), "root", timeout=0.05)
        self.assertEqual(status, "timeout")
        self.assertIn("0", error)
        self.assertIsNone(reply)

    def test_events_for_other_agents_are_ignored(self):
        frames = [
            {"frame": "event", "event": {"agent_id": "child", "kind": {"type": "agent.completed"}}},
            {"frame": "event", "event": {"agent_id": "root", "kind": {"type": "agent.completed"}}},
        ]
        status, _error, _reply = hb.wait_for_run_terminal(FakeClient(frames=frames), "root", timeout=5)
        self.assertEqual(status, "completed")


class OverallSuccessTests(unittest.TestCase):
    def test_full_pass_with_completed_workflow_is_success(self):
        result = {"workflow_status": "completed", "grade": {"passed": 8, "total": 8}}
        self.assertTrue(hb._overall_success(result))

    def test_timeout_is_never_success_even_with_a_full_grade(self):
        result = {"workflow_status": "timeout", "grade": {"passed": 8, "total": 8}}
        self.assertFalse(hb._overall_success(result))

    def test_partial_grade_is_not_success(self):
        result = {"workflow_status": "completed", "grade": {"passed": 7, "total": 8}}
        self.assertFalse(hb._overall_success(result))

    def test_contract_violation_overrides_a_full_grade(self):
        result = {"workflow_status": "completed", "grade": {"passed": 8, "total": 8},
                  "contract_violations": {"changed": ["tests/test_x.py"]}}
        self.assertFalse(hb._overall_success(result))

    def test_missing_grade_is_not_success(self):
        self.assertFalse(hb._overall_success({"workflow_status": "completed"}))


class FrozenContractTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-frozen-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        (self.tmp / "tests").mkdir()
        (self.tmp / "tests" / "test_x.py").write_text("def test_x(): assert True\n")
        (self.tmp / "CONTRACT.md").write_text("the contract\n")
        (self.tmp / "solution.py").write_text("x = 1\n")

    def test_snapshot_captures_visible_tests_and_contract_but_not_solution(self):
        digest = hb.snapshot_frozen_files(self.tmp)
        self.assertIn("tests/test_x.py", digest)
        self.assertIn("CONTRACT.md", digest)
        self.assertNotIn("solution.py", digest)

    def test_no_violation_when_frozen_files_are_untouched(self):
        before = hb.snapshot_frozen_files(self.tmp)
        (self.tmp / "solution.py").write_text("x = 2\n")
        after = hb.snapshot_frozen_files(self.tmp)
        self.assertIsNone(hb.frozen_violations(before, after))

    def test_editing_a_visible_test_is_reported_as_a_violation(self):
        before = hb.snapshot_frozen_files(self.tmp)
        (self.tmp / "tests" / "test_x.py").write_text("def test_x(): assert False\n")
        after = hb.snapshot_frozen_files(self.tmp)
        violations = hb.frozen_violations(before, after)
        self.assertIsNotNone(violations)
        self.assertIn("tests/test_x.py", violations["changed"])

    def test_deleting_the_contract_is_reported_as_removed(self):
        before = hb.snapshot_frozen_files(self.tmp)
        (self.tmp / "CONTRACT.md").unlink()
        after = hb.snapshot_frozen_files(self.tmp)
        violations = hb.frozen_violations(before, after)
        self.assertIn("CONTRACT.md", violations["removed"])


class ClaudeReceiptAttributionTests(unittest.TestCase):
    def test_single_claude_condition_attributes_to_the_root_agent(self):
        condition = hb.CONDITIONS["aether-single-claude"]
        captures = [{"capture_id": "123", "receipt": {"type": "result"}}]
        attributed = hb.attribute_claude_receipts(condition, "root-abc", captures)
        self.assertEqual(attributed[0]["agent_id"], "root-abc")

    def test_coordinator_condition_leaves_attribution_unknown(self):
        condition = hb.CONDITIONS["aether-coordinator-team"]
        captures = [{"capture_id": "123", "receipt": None}]
        attributed = hb.attribute_claude_receipts(condition, "root-abc", captures)
        self.assertIsNone(attributed[0]["agent_id"])
        self.assertIsNone(attributed[0]["receipt"], "a killed/failed process must still show receipt: None, not be dropped")


class SocketPathLengthTests(unittest.TestCase):
    def test_short_path_is_accepted(self):
        hb._assert_socket_path_ok("/home/x/aether/target/hb/abc123/aetherd.sock")

    def test_overlong_path_raises(self):
        with self.assertRaises(AssertionError):
            hb._assert_socket_path_ok("/" + ("a" * 120) + "/aetherd.sock")


class BinaryProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-provenance-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_pin_binary_copies_once_and_hash_matches_the_copy(self):
        source = self.tmp / "aether-src"
        source.write_bytes(b"pretend-binary-bytes")
        output_dir = self.tmp / "out"
        output_dir.mkdir()
        pinned_path, digest = hb.pin_binary(str(source), str(output_dir))
        self.assertTrue(os.path.isfile(pinned_path))
        self.assertEqual(digest, hb.sha256_file(pinned_path))
        source.write_bytes(b"a rebuild changed the bytes")
        self.assertEqual(hb.sha256_file(pinned_path), digest, "a later rebuild must not retroactively change the pin")

    def test_pin_binary_rejects_a_missing_source(self):
        with self.assertRaises(AssertionError):
            hb.pin_binary(str(self.tmp / "does-not-exist"), str(self.tmp))

    def test_native_cli_provenance_reports_none_for_an_unresolvable_command(self):
        info = hb.native_cli_provenance("hb-definitely-not-a-real-cli")
        self.assertIsNone(info["path"])
        self.assertIsNone(info["sha256"])


class ClaudeReceiptProxyFunctionalTests(unittest.TestCase):
    """Exercises the generated proxy script for real (no real claude CLI)."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-proxy-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def _fake_real_claude(self, body):
        path = self.tmp / "fake-claude"
        path.write_text(f"#!/usr/bin/env python3\n{body}\n")
        os.chmod(path, 0o755)
        return str(path)

    def test_proxy_forwards_stdout_and_captures_the_result_receipt(self):
        real = self._fake_real_claude(
            "import json, sys\n"
            "print(json.dumps({'type': 'system'}))\n"
            "print(json.dumps({'type': 'result', 'usage': {'input_tokens': 3}}))\n"
        )
        env = dict(os.environ)
        receipts = hb.install_claude_receipt_proxy(env, str(self.tmp), real)
        proxy = subprocess.run([os.path.join(env["PATH"].split(os.pathsep)[0], "claude"), "-p"],
                                capture_output=True, text=True, env=env, timeout=10, check=False)
        self.assertEqual(proxy.returncode, 0)
        lines = [json.loads(line) for line in proxy.stdout.splitlines() if line.strip()]
        self.assertEqual(lines[-1]["type"], "result")
        captures = hb.read_claude_receipts(receipts)
        self.assertEqual(len(captures), 1)
        self.assertEqual(captures[0]["receipt"]["usage"]["input_tokens"], 3)

    def test_proxy_leaves_a_none_receipt_when_no_result_event_is_ever_emitted(self):
        real = self._fake_real_claude("import sys\nprint('no json here')\nsys.exit(1)\n")
        env = dict(os.environ)
        receipts = hb.install_claude_receipt_proxy(env, str(self.tmp), real)
        proxy = subprocess.run([os.path.join(env["PATH"].split(os.pathsep)[0], "claude"), "--print"],
                                capture_output=True, text=True, env=env, timeout=10, check=False)
        self.assertEqual(proxy.returncode, 1, "the proxy must preserve the wrapped process's real exit code")
        captures = hb.read_claude_receipts(receipts)
        self.assertEqual(len(captures), 1)
        self.assertIsNone(captures[0]["receipt"])


class GitProvenanceTests(unittest.TestCase):
    def test_git_provenance_never_includes_diff_content(self):
        info = hb.git_provenance(hb.REPO_ROOT)
        self.assertIn("head", info)
        self.assertIn("dirty", info)
        self.assertIsInstance(info["changed_paths"], list)
        blob = json.dumps(info)
        self.assertNotIn("\n+", blob, "provenance must record paths only, never diff hunks")


class CompletionProtocolRegressionTests(unittest.TestCase):
    def setUp(self):
        self.default = hb.CONDITIONS['aether-coordinator-default']
        self.team = hb.CONDITIONS['aether-coordinator-team']

    def team_tree(self):
        return {'nodes': {'root': node('root', 'completed', ['a', 'b']),
                          'a': node('a', 'completed', effort='high'),
                          'b': node('b', 'completed', effort='high')}}

    def test_default_reply_inbox_and_completed_children_normalizes_to_success(self):
        tree = self.team_tree()
        tree['nodes']['root'] = node('root', 'waiting', ['a', 'b'])
        status = hb.normalized_workflow_status(self.default, 'waiting', None, 'done', tree, 'root')
        self.assertEqual(status, 'completed')
        self.assertFalse(hb.protocol_violations(self.default, tree, 'root'))
        self.assertTrue(hb._overall_success({'workflow_status': status, 'grade': {'passed': 1, 'total': 1}}))

    def test_waiting_for_tasks_or_time_is_not_quiescent_or_complete(self):
        for on in ('tasks', 'time'):
            with self.subTest(on=on):
                tree = self.team_tree()
                tree['nodes']['root']['agent']['status'] = {'state': 'waiting', 'condition': {'on': on}}
                self.assertFalse(hb.tree_is_quiescent(tree, 'root'))
                self.assertEqual(hb.normalized_workflow_status(self.default, 'waiting', None, 'done', tree, 'root'), 'timeout')

    def test_empty_reply_or_error_does_not_complete_default(self):
        tree = {'nodes': {'root': node('root', 'waiting')}}
        for reply, error in (('', None), (None, None), ('done', 'failed')):
            self.assertNotEqual(hb.normalized_workflow_status(self.default, 'waiting', error, reply, tree, 'root'), 'completed')

    def test_team_requires_exact_two_completed_correctly_staffed_direct_children(self):
        self.assertEqual(hb.protocol_violations(self.team, self.team_tree(), 'root'), [])
        for variant in ('zero', 'one', 'three', 'failed', 'effort', 'model', 'grandchild'):
            tree = self.team_tree()
            if variant in ('zero', 'one'):
                for child in (['a', 'b'] if variant == 'zero' else ['b']):
                    del tree['nodes'][child]
                    tree['nodes']['root']['children'].remove(child)
            elif variant in ('three', 'grandchild'):
                tree['nodes']['c'] = node('c', 'completed', effort='high')
                tree['nodes']['root' if variant == 'three' else 'a']['children'].append('c')
            elif variant == 'failed':
                tree['nodes']['a']['agent']['status']['state'] = 'failed'
            elif variant == 'effort':
                tree['nodes']['a']['agent']['config']['effort'] = 'low'
            else:
                tree['nodes']['a']['agent']['model']['model'] = 'other'
            with self.subTest(variant=variant):
                violations = hb.protocol_violations(self.team, tree, 'root')
                self.assertTrue(violations)
                self.assertFalse(hb._overall_success({'workflow_status': 'completed', 'grade': {'passed': 1, 'total': 1},
                                                     'protocol_violations': violations}))

    def test_solo_rejects_any_descendant(self):
        self.assertTrue(hb.protocol_violations(hb.CONDITIONS['aether-single-codex'], self.team_tree(), 'root'))

    def test_all_conditions_share_workspace_and_no_network_boundary(self):
        for condition in hb.CONDITIONS.values():
            self.assertIn(hb.COMMON_SUFFIX, condition['suffix'])

    def test_socket_timeout_becomes_timeout_not_exception(self):
        client = Mock()
        client.next_frame.side_effect = TimeoutError('socket timed out')
        self.assertEqual(hb.wait_for_run_terminal(client, 'root', 1)[0], 'timeout')


def blocked_trial_events(root_id='agt_root', tool_error=True, idle=True, task_assigned=True):
    """A trimmed excerpt shaped like the real recorded events (same
    `seq`/`agent_id`/`kind.type` fields, real event ordering) for the two
    pilot trials that hit a refused `delegate` with zero children created:
    target/hb/1e710e857c and target/hb/179f0f413e in the 2026-09-15 pilot
    run's captured events.json (host missing the `socat` dependency).
    """
    events = [
        {'seq': 2, 'agent_id': root_id, 'kind': {'type': 'agent.created', 'role': 'coordinator'}},
        {'seq': 3, 'agent_id': root_id, 'kind': {'type': 'task.created', 'objective': 'fix the ledger'}},
    ]
    if task_assigned:
        events.append({'seq': 4, 'agent_id': root_id, 'kind': {'type': 'task.assigned'}})
    events += [
        {'seq': 8, 'agent_id': root_id, 'kind': {'type': 'task.started'}},
        {'seq': 18, 'agent_id': root_id, 'kind': {'type': 'tool.succeeded', 'call': 'call_read',
            'outcome': {'content': 'CONTRACT.md ...', 'is_error': False}}},
    ]
    if tool_error:
        events.append({'seq': 42, 'agent_id': root_id, 'kind': {'type': 'tool.succeeded', 'call': 'call_delegate',
            'outcome': {'content': 'Cannot delegate to model `claude-code/sonnet`: unsupported engine '
                                   'configuration: missing: socat.', 'is_error': True}}})
    if idle:
        events.append({'seq': 77, 'agent_id': root_id, 'kind': {'type': 'agent.waiting', 'condition': {'on': 'inbox'}}})
    events += [
        {'seq': 80, 'agent_id': root_id, 'kind': {'type': 'agent.cancelled'}},
        {'seq': 82, 'agent_id': root_id, 'kind': {'type': 'task.cancelled'}},
    ]
    return events


class TrialOrderTests(unittest.TestCase):
    CASES = ["a", "b", "c"]
    CONDITIONS = ["x", "y", "z", "w"]

    def test_every_cell_runs_once_per_run_and_runs_are_outer(self):
        order = hb.trial_order(self.CASES, self.CONDITIONS, 2)
        self.assertEqual(len(order), 3 * 4 * 2)
        self.assertEqual(len(set(order)), len(order), "no cell may be scheduled twice")
        run_indexes = [run for _, _, run in order]
        self.assertEqual(run_indexes, sorted(run_indexes), "all run-1 cells precede run-2 cells")
        self.assertEqual({(c, k) for c, k, _ in order}, {(c, k) for c in self.CASES for k in self.CONDITIONS})

    def test_condition_positions_are_balanced_within_a_run(self):
        order = hb.trial_order(self.CASES, self.CONDITIONS, 1)
        first_per_case = [order[i * 4][1] for i in range(3)]
        self.assertEqual(len(set(first_per_case)), 3, f"a condition led more than one case: {first_per_case}")
        for i in range(3):
            block = [key for _, key, _ in order[i * 4:(i + 1) * 4]]
            self.assertEqual(sorted(block), sorted(self.CONDITIONS))

    def test_rotation_differs_between_runs_and_is_deterministic(self):
        runs = hb.trial_order(self.CASES, self.CONDITIONS, 4)
        per_run = [[cell[:2] for cell in runs[i * 12:(i + 1) * 12]] for i in range(4)]
        self.assertEqual(len({tuple(block) for block in per_run}), 4, "each run should use a distinct order")
        self.assertEqual(runs, hb.trial_order(self.CASES, self.CONDITIONS, 4))
        leads = [block[0][1] for block in per_run]
        self.assertEqual(sorted(leads), sorted(self.CONDITIONS), "over four runs every condition leads once")

    def test_single_cell_and_bad_input(self):
        self.assertEqual(hb.trial_order(["a"], ["x"], 1), [("a", "x", 1)])
        with self.assertRaises(AssertionError):
            hb.trial_order([], ["x"], 1)
        with self.assertRaises(AssertionError):
            hb.trial_order(["a", "a"], ["x"], 1)

    def test_seeded_pair_alternates_and_replays(self):
        first = hb.trial_order(["a"], ["native", "aether"], 3, seed=17)
        self.assertEqual(first, hb.trial_order(["a"], ["native", "aether"], 3, seed=17))
        pairs = [first[index:index + 2] for index in range(0, len(first), 2)]
        self.assertEqual([pair[0][1] for pair in pairs],
                         [pairs[0][0][1], pairs[0][1][1], pairs[0][0][1]])
        self.assertTrue(all({item[1] for item in pair} == {"native", "aether"} for pair in pairs))


class BlockedTrialClassificationTests(unittest.TestCase):
    """A blocked trial is a host/environment refusal (e.g. missing `socat`
    breaking `delegate`), not a model correctness failure; it must be kept
    distinct from `failed` so it does not silently pollute pass-rate means.
    """

    def test_refused_delegate_with_zero_children_and_inbox_idle_is_blocked(self):
        result = {'descendant_count': 0}
        self.assertTrue(hb.is_blocked_trial(blocked_trial_events(), result, 'agt_root'))

    def test_children_were_created_is_never_blocked_even_after_a_tool_error(self):
        result = {'descendant_count': 2}
        self.assertFalse(hb.is_blocked_trial(blocked_trial_events(), result, 'agt_root'))

    def test_inbox_idle_without_a_preceding_tool_error_is_not_blocked(self):
        result = {'descendant_count': 0}
        self.assertFalse(hb.is_blocked_trial(blocked_trial_events(tool_error=False), result, 'agt_root'))

    def test_tool_error_that_never_reaches_inbox_idle_is_not_blocked(self):
        result = {'descendant_count': 0}
        self.assertFalse(hb.is_blocked_trial(blocked_trial_events(idle=False), result, 'agt_root'))

    def test_never_assigned_a_task_is_not_blocked(self):
        result = {'descendant_count': 0}
        self.assertFalse(hb.is_blocked_trial(blocked_trial_events(task_assigned=False), result, 'agt_root'))

    def test_tool_error_after_the_idle_wait_does_not_count_as_preceding(self):
        events = blocked_trial_events(tool_error=False)
        events.append({'seq': 90, 'agent_id': 'agt_root', 'kind': {'type': 'tool.succeeded', 'call': 'late',
            'outcome': {'content': 'boom', 'is_error': True}}})
        result = {'descendant_count': 0}
        self.assertFalse(hb.is_blocked_trial(events, result, 'agt_root'))

    def test_transport_level_tool_failed_also_counts_as_a_tool_error(self):
        events = blocked_trial_events(tool_error=False)
        events.insert(-1, {'seq': 41, 'agent_id': 'agt_root', 'kind': {'type': 'tool.failed', 'error': 'timeout'}})
        result = {'descendant_count': 0}
        self.assertTrue(hb.is_blocked_trial(events, result, 'agt_root'))

    def test_a_different_agents_tool_error_does_not_block_the_root(self):
        events = blocked_trial_events(tool_error=False)
        events.insert(-1, {'seq': 41, 'agent_id': 'agt_other', 'kind': {'type': 'tool.succeeded', 'call': 'x',
            'outcome': {'content': 'boom', 'is_error': True}}})
        result = {'descendant_count': 0}
        self.assertFalse(hb.is_blocked_trial(events, result, 'agt_root'))

    def test_trial_status_prefers_blocked_over_failed_and_never_both(self):
        self.assertEqual(hb._trial_status({'blocked': True, 'success': False}), 'blocked')
        self.assertEqual(hb._trial_status({'blocked': False, 'success': False}), 'failed')
        self.assertEqual(hb._trial_status({'blocked': False, 'success': True}), 'completed')


class ProxyProbeRegressionTests(ClaudeReceiptProxyFunctionalTests):
    def test_auth_and_version_probes_produce_no_attempt_receipts(self):
        real = self._fake_real_claude("print('probe response')")
        env = dict(os.environ)
        receipts = hb.install_claude_receipt_proxy(env, str(self.tmp), real)
        proxy = str(self.tmp / 'bin/claude')
        for args in (['--version'], ['auth', 'status']):
            output = subprocess.run([proxy, *args], env=env, capture_output=True, text=True, timeout=10, check=True)
            self.assertEqual(output.stdout, 'probe response\n')
        self.assertEqual(hb.read_claude_receipts(receipts), [])

    def test_receipt_drops_response_text_but_preserves_accounting(self):
        event = {'type': 'result', 'subtype': 'success', 'result': 'SECRET', 'errors': ['SECRET'],
                 'session_id': 'session', 'modelUsage': {'model': {'inputTokens': 12}},
                 'usage': {'input_tokens': 12, 'unexpected': 'SECRET'}, 'is_error': False}
        real = self._fake_real_claude(f'import json\nprint(json.dumps({event!r}))')
        env = dict(os.environ)
        receipts = hb.install_claude_receipt_proxy(env, str(self.tmp), real)
        output = subprocess.run([str(self.tmp / 'bin/claude'), '-p'], env=env, capture_output=True, text=True, timeout=10, check=True)
        self.assertIn('SECRET', output.stdout, 'proxy must not modify the engine stream')
        captures = hb.read_claude_receipts(receipts)
        self.assertNotIn('SECRET', json.dumps(captures))
        self.assertEqual(captures[0]['receipt']['modelUsage']['model']['inputTokens'], 12)

if __name__ == "__main__":
    unittest.main()
