"""Check ordinary delegation requests and native web access in the resulting children."""
import argparse
import json
from pathlib import Path
import tempfile

from aether_client import AetherClient, init_git_workspace, isolated_env, start_daemon, stop_daemon
from natural_language import check

CASES = [
    {'name': 'codex-to-sonnet-web', 'model': 'codex/gpt-6-luna',
     'prompt': 'Have two Sonnet agents each search online for the official Rust book introduction, then give me their links.',
     'children': {'count': 2, 'provider': 'claude-code', 'model': 'sonnet'}},
    {'name': 'claude-to-luna-web', 'model': 'claude-code/sonnet',
     'prompt': 'Have two Luna low agents each search online for the official Rust book introduction, then give me their links.',
     'children': {'count': 2, 'provider': 'codex', 'model': 'gpt-6-luna', 'effort': 'low'}},
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', default='target/debug/aether')
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    results = []
    for case in CASES:
        with tempfile.TemporaryDirectory(prefix='aether-child-web-') as root:
            env = isolated_env(root)
            workspace = Path(root) / 'workspace'
            workspace.mkdir()
            init_git_workspace(workspace)
            start_daemon(binary, env, str(workspace))
            try:
                with AetherClient(env['AETHER_SOCKET']) as client:
                    client.subscribe()
                    result = check(client, case, 240)
                    native_calls = []
                    for child in result['agents']:
                        if child.get('parent') != result['root']:
                            continue
                        if child['scope']['network'] != 'allowed':
                            result['errors'].append(f"{child['id']}: network scope was not inherited")
                        calls = [event['kind']['call'] for event in client.events_seen
                                 if event.get('agent_id') == child['id'] and event['kind']['type'] == 'tool.requested']
                        web = [call for call in calls if call['name'] in ('web_search', 'WebSearch', 'WebFetch')]
                        if not web:
                            result['errors'].append(f"{child['id']}: no native web call recorded")
                        native_calls.extend(web)
                    result['native_web_calls'] = native_calls
                    result['passed'] = not result['errors']
                    results.append(result)
                    Path(args.out).write_text(json.dumps(results, indent=2) + '\n')
                    print(json.dumps({'name': case['name'], 'passed': result['passed'], 'errors': result['errors']}), flush=True)
            finally:
                stop_daemon(binary, env)
    raise SystemExit(0 if all(row['passed'] for row in results) else 1)


if __name__ == '__main__':
    main()
