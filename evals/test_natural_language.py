"""Keep live tree acceptance strict about lineage, models, effort and outcomes."""
import copy
import subprocess
import tempfile
import unittest
from pathlib import Path

from aether_client import init_git_workspace
from natural_language import check, first_turn_errors, tool_failed, tree_errors, validate_case_schema


class FirstTurnTests(unittest.TestCase):
    def event(self, agent, kind, **fields):
        return {'agent_id': agent, 'kind': {'type': kind, **fields}}

    def test_accepts_three_direct_children_in_the_first_turn(self):
        events = [self.event('root', 'agent.started', turn=1)]
        events += [self.event(str(n), 'agent.created', parent='root') for n in range(3)]
        events += [self.event('root', 'agent.turn_completed', turn=1)]
        self.assertEqual(first_turn_errors(events, 'root', 3), [])

    def test_later_children_cannot_repair_a_failed_first_turn(self):
        events = [self.event('root', 'agent.started', turn=1),
                  self.event('one', 'agent.created', parent='root'),
                  self.event('root', 'agent.turn_completed', turn=1),
                  self.event('root', 'agent.started', turn=2),
                  self.event('two', 'agent.created', parent='root'),
                  self.event('three', 'agent.created', parent='root')]
        self.assertTrue(first_turn_errors(events, 'root', 3))

    def test_missing_boundaries_and_grandchildren_do_not_count(self):
        self.assertTrue(first_turn_errors([], 'root', 3))
        self.assertTrue(first_turn_errors([self.event('root', 'agent.started', turn=2)], 'root', 3))
        events = [self.event('root', 'agent.started', turn=1),
                  self.event('one', 'agent.created', parent='root'),
                  self.event('two', 'agent.created', parent='one'),
                  self.event('three', 'agent.created', parent='one'),
                  self.event('root', 'agent.turn_completed', turn=1)]
        self.assertTrue(first_turn_errors(events, 'root', 3))


class FirstTurnObservationTests(unittest.TestCase):
    def test_waiting_state_does_not_end_observation_before_turn_event(self):
        root = {'id': 'root', 'run': 'run', 'inbox_cursor': 1,
                'status': {'state': 'running'}}
        children = [{'id': str(n), 'parent': 'root', 'model': {'provider': 'mock', 'model': 'm'},
                     'config': {}, 'status': {'state': 'completed', 'summary': 'Read README'}}
                    for n in range(3)]
        events = [{'agent_id': 'root', 'kind': {'type': 'agent.started', 'turn': 1}}]
        events += [{'agent_id': child['id'], 'kind': {'type': 'agent.created', 'parent': 'root'}}
                   for child in children]
        events += [{'agent_id': 'root', 'kind': {'type': 'agent.waiting'}},
                   {'agent_id': 'root', 'kind': {'type': 'agent.turn_completed', 'turn': 1}}]

        class Client:
            def __init__(self):
                self.events_seen = []

            def create_agent(self, **kwargs):
                return root

            def send(self, *args):
                pass

            def request(self, command):
                return {'data': [root, *children]}

            def next_frame(self, timeout):
                event = events.pop(0)
                self.events_seen.append(event)
                if event['kind']['type'] == 'agent.waiting':
                    root['status'] = {'state': 'waiting'}
                return {'frame': 'event', 'event': event}

        case = {'name': 'fixture', 'model': 'mock/m', 'prompt': 'Start three researchers',
                'first_turn_children': 3, 'tree': {'count': 3, 'provider': 'mock', 'model': 'm'}}
        result = check(Client(), case, 5)
        self.assertTrue(result['passed'], result['errors'])
        self.assertEqual(result['lifecycle'][-1]['kind']['type'], 'agent.turn_completed')


class TreeAcceptanceTests(unittest.TestCase):
    def test_error_outcome_counts_even_when_execution_event_succeeded(self):
        self.assertTrue(tool_failed({"type": "tool.failed"}))
        self.assertTrue(tool_failed({"type": "tool.succeeded", "outcome": {"is_error": True}}))
        self.assertFalse(tool_failed({"type": "tool.succeeded", "outcome": {"is_error": False}}))
        self.assertFalse(tool_failed({"type": "agent.message"}))

    def setUp(self):
        self.expected = {
            "min_count": 2, "max_count": 4, "provider": "codex", "model": "sol",
            "children": {"count": 1, "provider": "codex", "model": "luna", "effort": "low"},
        }
        self.agents = []
        for number in range(2):
            parent = f"sol-{number}"
            for identity, ancestor, model in ((parent, "root", "sol"), (f"luna-{number}", parent, "luna")):
                self.agents.append({
                    "id": identity, "parent": ancestor,
                    "model": {"provider": "codex", "model": model}, "config": {"effort": "low"},
                    "status": {"state": "completed", "summary": "Read the documentation."},
                })

    def test_accepts_bounded_few_with_completed_work(self):
        self.assertEqual(tree_errors(self.agents, "root", self.expected), [])

    def test_rejects_same_count_under_wrong_parent(self):
        self.agents[1]["parent"] = "sol-1"
        self.assertTrue(tree_errors(self.agents, "root", self.expected))

    def test_rejects_invisible_native_agents(self):
        self.assertTrue(tree_errors([], "root", self.expected))

    def test_in_progress_allows_pending_children_but_stops_extra_generations(self):
        self.assertEqual(tree_errors([], "root", self.expected, in_progress=True), [])
        self.agents[0]["status"] = {"state": "running"}
        self.assertEqual(tree_errors(self.agents, "root", self.expected, in_progress=True), [])
        extra = copy.deepcopy(self.agents[1])
        extra.update(id="unexpected", parent="luna-0")
        self.agents.append(extra)
        self.assertTrue(tree_errors(self.agents, "root", self.expected, in_progress=True))

    def test_rejects_wrong_model_effort_and_missing_work(self):
        changes = [
            ("model", {"provider": "anthropic", "model": "sonnet"}),
            ("config", {"effort": "high"}),
            ("status", {"state": "failed", "summary": "Failed"}),
            ("status", {"state": "completed", "summary": ""}),
        ]
        for key, value in changes:
            with self.subTest(key=key, value=value):
                agents = copy.deepcopy(self.agents)
                agents[1][key] = value
                self.assertTrue(tree_errors(agents, "root", self.expected))


class ValidateCaseSchemaTests(unittest.TestCase):
    def valid_case(self, **overrides):
        case = {
            "name": "a-case", "model": "codex/gpt-6-luna", "prompt": "do something",
            "children": {"count": 1, "provider": "codex", "model": "gpt-6-luna"},
        }
        case.update(overrides)
        return case

    def test_accepts_a_well_formed_case(self):
        self.assertEqual(validate_case_schema(self.valid_case()), [])

    def test_rejects_a_missing_name(self):
        case = self.valid_case()
        del case["name"]
        errors = validate_case_schema(case)
        self.assertTrue(any("name" in error for error in errors))

    def test_rejects_non_dict_children(self):
        errors = validate_case_schema(self.valid_case(children=4))
        self.assertTrue(any("must be an object" in error for error in errors))

    def test_rejects_a_string_count(self):
        errors = validate_case_schema(
            self.valid_case(children={"count": "1", "provider": "codex", "model": "gpt-6-luna"})
        )
        self.assertTrue(any("'count' must be an integer" in error for error in errors))

    def test_rejects_a_non_list_events_required(self):
        errors = validate_case_schema(self.valid_case(events_required="checkpoint.created"))
        self.assertTrue(any("events_required" in error for error in errors))


class InitGitWorkspaceTests(unittest.TestCase):
    def git(self, workspace, *args):
        return subprocess.run(["git", *args], cwd=workspace, check=True, capture_output=True, text=True).stdout

    def test_pins_a_stable_identity_and_produces_a_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            (workspace / "README.md").write_text("# fixture\n")
            init_git_workspace(workspace)

            log = self.git(workspace, "log", "-1", "--format=%an <%ae>")
            self.assertEqual(log.strip(), "Aether Eval <eval@aether.test>")
            head = self.git(workspace, "rev-parse", "HEAD")
            self.assertTrue(head.strip())

    def test_works_on_an_empty_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            init_git_workspace(workspace)
            head = self.git(workspace, "rev-parse", "HEAD")
            self.assertTrue(head.strip())


if __name__ == "__main__":
    unittest.main()
