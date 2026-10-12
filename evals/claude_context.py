"""Verify actual Claude context windows through Aether, including compaction."""
import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile

from aether_client import AetherClient, isolated_env, start_daemon, stop_daemon

# Observe result metadata without changing the CLI's input/output protocol.
# Each invocation writes its own file; prompts and bridge credentials are not logged.
PROXY = '''#!/usr/bin/env python3
import json, os, subprocess, sys
from pathlib import Path
args = sys.argv[1:]
if "-p" not in args:
    os.execv({claude!r}, [{claude!r}, *args])
child = subprocess.Popen([{claude!r}, *args], stdout=subprocess.PIPE)
try:
    for line in child.stdout:
        sys.stdout.buffer.write(line)
        sys.stdout.buffer.flush()
        try:
            event = json.loads(line)
        except (ValueError, UnicodeDecodeError):
            continue
        if event.get("type") == "result":
            row = {{"resumed": "--resume" in args,
                   "summary": "--tools" in args and args[args.index("--tools") + 1] == "",
                   "model": args[args.index("--model") + 1] if "--model" in args else "default",
                   "is_error": event.get("is_error"), "usage": event.get("modelUsage", {{}})}}
            Path({records!r}, str(os.getpid()) + ".json").write_text(json.dumps(row))
    code = child.wait()
finally:
    if child.poll() is None:
        child.terminate()
        child.wait()
sys.exit(code)
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', default='target/debug/aether')
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    claude = shutil.which('claude')
    if not claude:
        parser.error('Claude CLI is required')
    results = []
    for model in ['sonnet', 'fable', 'opus', 'default']:
        with tempfile.TemporaryDirectory(prefix='aether-context-window-') as directory:
            root = Path(directory)
            env = isolated_env(directory)
            workspace = root / 'workspace'
            workspace.mkdir()
            records = root / 'records'
            records.mkdir()
            wrappers = root / 'bin'
            wrappers.mkdir()
            proxy = wrappers / 'claude'
            proxy.write_text(PROXY.format(claude=claude, records=str(records)))
            proxy.chmod(0o755)
            env['PATH'] = str(wrappers) + os.pathsep + env['PATH']
            (Path(env['AETHER_CONFIG_DIR']) / 'config.toml').write_text(
                '[model]\ncompactor = "claude-code/sonnet"\ncompactor_effort = "medium"\n')
            row = {'model': model, 'errors': []}
            try:
                start_daemon(binary, env, str(workspace))
                with AetherClient(env['AETHER_SOCKET']) as client:
                    agent = client.create_agent(model='claude-code/' + model, effort='low')
                    aid = agent['id']
                    client.subscribe(aid)
                    client.send(aid, 'Remember this validation fact: the project codename is Amber Finch. Reply noted.')
                    client.wait_for_reply(aid)
                    first_session = client.get_agent(aid)['engine_session']
                    client.send(aid, 'Keep the codename for later. Reply noted.')
                    client.wait_for_reply(aid)
                    if client.get_agent(aid)['engine_session'] != first_session:
                        row['errors'].append('Ordinary follow-up did not reuse the session')
                    client.compact(aid)
                    client.send(aid, 'What is the project codename? Reply with just the name.')
                    row['answer'] = client.wait_for_reply(aid)
                    row['agent'] = client.get_agent(aid)
                    if not client.find_event(aid, 'compaction.completed'):
                        row['errors'].append('No completed compaction recorded')
                    if row['agent']['engine_session'] == first_session:
                        row['errors'].append('Compaction did not replace the session')
                    if 'amber finch' not in row['answer'].lower():
                        row['errors'].append('Codename was not retained')
            except Exception as error:
                row['errors'].append(str(error))
            finally:
                stop_daemon(binary, env)
            row['invocations'] = [json.loads(path.read_text()) for path in sorted(records.glob('*.json'))]
            for mode in ['resumed', 'summary']:
                if not any(call[mode] for call in row['invocations']):
                    row['errors'].append(f'No native {mode} result observed')
            for call in row['invocations']:
                if call['is_error'] or not call['usage']:
                    row['errors'].append('Native invocation failed or omitted model usage')
                for name, usage in call['usage'].items():
                    if usage.get('contextWindow') != 200000:
                        row['errors'].append(f'{name}: unexpected context window {usage.get("contextWindow")}')
            row['passed'] = not row['errors']
            results.append(row)
            Path(args.out).write_text(json.dumps(results, indent=2) + '\n')
            print(json.dumps({key: row[key] for key in ['model', 'passed', 'errors']}), flush=True)
    raise SystemExit(0 if all(row['passed'] for row in results) else 1)


if __name__ == '__main__':
    main()
