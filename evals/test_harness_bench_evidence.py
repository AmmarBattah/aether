"""Offline durable benchmark-evidence and privacy regression tests."""
from contextlib import closing
import json
import os
import shutil
import sqlite3
import tempfile
import unittest
import subprocess
from types import SimpleNamespace
from unittest.mock import patch
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import harness_bench as hb
import harness_bench_assets as assets
from harness_bench_test_support import TEST_SCRATCH, node


class DurableEvidenceRegressionTests(unittest.TestCase):
    def test_git_provenance_preserves_status_columns_and_literal_paths(self):
        status = " M ARCHITECTURE.md\0?? spaced\nfile \0R  new name\0old name\0"
        with patch.object(assets.subprocess, 'run', side_effect=[
            SimpleNamespace(stdout=status), SimpleNamespace(stdout="abc123\n")
        ]) as run:
            evidence = assets.git_provenance('/repo')
        self.assertEqual(evidence, {'head': 'abc123', 'dirty': True,
                                  'changed_paths': ['ARCHITECTURE.md', 'spaced\nfile ', 'new name', 'old name']})
        self.assertEqual(run.call_args_list[0].args[0], ['git', 'status', '--porcelain=v1', '-z'])
        self.assertTrue(run.call_args_list[0].kwargs['check'])

    def test_git_failure_cannot_be_reported_as_clean_provenance(self):
        with patch.object(assets.subprocess, 'run', side_effect=subprocess.CalledProcessError(128, ['git'])):
            with self.assertRaises(subprocess.CalledProcessError):
                assets.git_provenance('/missing')

    def test_truncated_rename_provenance_fails_visibly(self):
        with patch.object(assets.subprocess, 'run', return_value=SimpleNamespace(stdout="R  new\0")):
            with self.assertRaisesRegex(ValueError, 'missing its source'):
                assets.git_provenance('/repo')

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix='evidence-'))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_real_sqlite_capture_contains_tail_and_allowlisted_snapshots(self):
        env = hb.isolated_env(str(self.tmp))
        events = [{'seq': 1, 'kind': {'type': 'agent.started'}, 'agent_id': 'root'},
                  {'seq': 2, 'kind': {'type': 'agent.cancelled'}, 'agent_id': 'root'}]
        agent = node('root', 'cancelled')['agent']
        snapshot = {'id': 'snap', 'agent_id': 'root', 'model': {'provider': 'codex', 'model': 'actual'},
                    'params': {'effort': 'medium', 'extensions': {'secret': 'SECRET'}},
                    'messages': ['SECRET'], 'tools': ['SECRET'], 'turn': 1}
        with closing(sqlite3.connect(Path(env['AETHER_DATA_DIR']) / 'aether.db')) as db:
            db.executescript('CREATE TABLE events (run TEXT, seq INTEGER, body TEXT);'
                             'CREATE TABLE agents (id TEXT, run TEXT, body TEXT);'
                             'CREATE TABLE snapshots (agent_id TEXT, created_at TEXT, body TEXT);')
            db.executemany('INSERT INTO events VALUES (?, ?, ?)', [('run', e['seq'], json.dumps(e)) for e in events])
            db.execute('INSERT INTO agents VALUES (?, ?, ?)', ('root', 'run', json.dumps(agent)))
            db.execute('INSERT INTO snapshots VALUES (?, ?, ?)', ('root', 'now', json.dumps(snapshot)))
            db.commit()
        result = {}
        with patch.object(hb.usage_module, 'parse_aether_events', return_value={'ok': True}) as parser:
            hb.capture_stopped_run(env, 'run', 'root', str(self.tmp), result, None)
        self.assertEqual(parser.call_args.args[0], events)
        self.assertEqual(json.loads((self.tmp / 'events.json').read_text()), events)
        self.assertEqual(json.loads((self.tmp / 'tree.json').read_text())['nodes']['root']['agent']['status']['state'], 'cancelled')
        raw = (self.tmp / 'snapshots.json').read_text()
        self.assertNotIn('SECRET', raw)
        self.assertEqual(json.loads(raw)[0]['effort'], 'medium')

    def test_hashes_include_code_starters_and_hidden_tests(self):
        hashes = hb.benchmark_hashes()
        self.assertIn('evals/harness_bench.py', hashes)
        for module in ('assets', 'config', 'outcomes', 'process', 'wait', 'test_support'):
            self.assertIn(f'evals/harness_bench_{module}.py', hashes)
        for suite in ('lifecycle', 'evidence'):
            self.assertIn(f'evals/test_harness_bench_{suite}.py', hashes)
        self.assertTrue(any('/starter/' in p for p in hashes))
        self.assertTrue(any('/hidden_tests/' in p for p in hashes))
        self.assertFalse(any('/reference/' in p for p in hashes))
        self.assertFalse(hb.verify_benchmark_hashes(hashes))
        modified = dict(hashes, **{'evals/harness_bench.py': 'changed'})
        self.assertEqual(hb.verify_benchmark_hashes(modified), ['evals/harness_bench.py'])

    def test_changed_inputs_block_paid_execution(self):
        with patch.object(hb, 'run_native_trial') as native:
            result = hb.run_trial('/fake', 'native-codex', {'id': 'fake', 'objective': 'x'}, 1,
                                  1, '/bin/true', str(self.tmp), expected_hashes={})
        native.assert_not_called()
        self.assertTrue(result['integrity_violations'])
        self.assertFalse(result['success'])

    def test_native_rollout_exposes_only_model_and_effort(self):
        session_dir = self.tmp / 'sessions/2026/09/15'
        session_dir.mkdir(parents=True)
        thread_id = '123456-abcd'
        (session_dir / f'rollout-{thread_id}.jsonl').write_text(json.dumps({
            'type': 'turn_context', 'payload': {'model': 'gpt-test', 'effort': 'medium', 'prompt': 'SECRET'}}))
        metadata = hb.native_resolved_metadata(json.dumps({'type': 'thread.started', 'thread_id': thread_id}),
                                               {'CODEX_HOME': str(self.tmp)}, False)
        self.assertEqual(metadata[0]['model'], 'gpt-test')
        self.assertEqual(metadata[0]['effort'], 'medium')
        self.assertNotIn('SECRET', json.dumps(metadata))

    def test_config_default_allowlist_does_not_leak_tokens(self):
        (self.tmp / 'config.toml').write_text('model="gpt-test"\nmodel_reasoning_effort="medium"\napi_key="SECRET"\n')
        with patch.dict(os.environ, {'CODEX_HOME': str(self.tmp)}):
            data = hb.native_configured_defaults('codex')
        self.assertEqual(data['model_reasoning_effort'], 'medium')
        self.assertNotIn('SECRET', json.dumps(data))


if __name__ == "__main__":
    unittest.main()
