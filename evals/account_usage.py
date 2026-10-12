"""Check native account quota reports through the shared provider projection."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import tempfile

from aether_client import AetherClient, init_git_workspace, isolated_env, start_daemon, stop_daemon


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--sandbox-bin')
    parser.add_argument('--models', nargs='+', default=['codex/gpt-6-luna', 'claude-code/haiku'],
                        help='Models to probe; defaults to Luna and Haiku.')
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    output = Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)
    result = {'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(), 'results': []}
    with tempfile.TemporaryDirectory(prefix='aether-account-usage-') as directory:
        env = isolated_env(directory)
        if args.sandbox_bin:
            env['PATH'] = str(Path(args.sandbox_bin).resolve()) + os.pathsep + env['PATH']
        workspace = Path(directory) / 'workspace'
        workspace.mkdir()
        init_git_workspace(workspace)
        client = None
        try:
            start_daemon(binary, env, str(workspace))
            client = AetherClient(env['AETHER_SOCKET'])
            client.subscribe(after=0)
            for model in args.models:
                row = {'model': model, 'errors': []}
                try:
                    agent = client.create_agent(model=model)
                    client.send(agent['id'], 'Reply with a brief greeting.')
                    row['reply'] = client.wait_for_reply(agent['id'], timeout=180)
                    provider = model.split('/', 1)[0]
                    accounts = client.request({'cmd': 'providers'})['data']
                    account = next(account for account in accounts if account['provider'] == provider)
                    row['usage'] = account['health'].get('usage')
                    reports = [event['kind'].get('usage') for event in client.events_seen
                               if event['kind']['type'] == 'provider.quota'
                               and event['kind']['provider'] == provider]
                    if not row['usage'] or not row['usage'].get('windows'):
                        row['errors'].append('Native provider did not expose a normalized account report')
                    elif row['usage'] not in reports:
                        row['errors'].append('Provider projection differs from the durable usage event')
                    else:
                        row['reported_windows'] = len(row['usage']['windows'])
                except Exception as error:
                    row['errors'].append(str(error))
                row['passed'] = not row['errors']
                result['results'].append(row)
                output.write_text(json.dumps(result, indent=2) + '\n')
                print(json.dumps({'model': model, 'passed': row['passed'], 'errors': row['errors']}), flush=True)
            accounts = client.request({'cmd': 'providers'})['data']
            for row in result['results']:
                provider = row['model'].split('/', 1)[0]
                account = next(account for account in accounts if account['provider'] == provider)
                if not row.get('usage') or account['health'].get('usage') != row['usage']:
                    row['errors'].append('Final fleet projection did not retain the account report')
                    row['passed'] = False
            output.write_text(json.dumps(result, indent=2) + '\n')
        finally:
            if client is not None:
                client.close()
            stop_daemon(binary, env)
    raise SystemExit(0 if all(row['passed'] for row in result['results']) else 1)


if __name__ == '__main__':
    main()
