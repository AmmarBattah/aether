"""Verify native web tools and network defaults through isolated Aether daemons."""
import argparse
from datetime import datetime, timezone
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from http.client import HTTPConnection
import threading
import json
from pathlib import Path
import tempfile

from aether_client import AetherClient, init_git_workspace, isolated_env, start_daemon, stop_daemon, protocol
from natural_language import tool_failed

SEARCH_PROMPT = 'Search the web for the official Rust book and give me a link to its introduction.'
CASES = [
    {'name': 'codex-default-web', 'model': 'codex/gpt-6-luna', 'tool': 'web_search'},
    {'name': 'claude-default-web', 'model': 'claude-code/sonnet', 'tool': 'WebSearch'},
    {'name': 'codex-configured-denial', 'model': 'codex/gpt-6-luna', 'denied': True},
    {'name': 'claude-configured-denial', 'model': 'claude-code/sonnet', 'denied': True},
    {'name': 'codex-sandbox-network', 'model': 'codex/gpt-6-luna', 'ask': True,
     'prompt': 'Use curl to check https://doc.rust-lang.org/book/ and report its HTTP status.', 'tool': 'bash'},
]
CASES += [{**case, 'name': case['name'] + '-resumed', 'resume': True}
          for case in CASES if not case.get('ask')]


def check(binary, case, timeout):
    with tempfile.TemporaryDirectory(prefix='aether-native-web-') as root:
        env = isolated_env(root)
        workspace = Path(root) / 'workspace'
        workspace.mkdir()
        init_git_workspace(workspace)
        if case.get('denied'):
            (Path(env['AETHER_CONFIG_DIR']) / 'config.toml').write_text('[network]\ndefault = "denied"\n')
        requests = []
        class ProbeHandler(BaseHTTPRequestHandler):
            def do_GET(self):
                requests.append(self.path)
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b"Aether network denial probe was reached")

            def log_message(self, *_args):
                pass  # Requests are recorded above, without noisy HTTP logs.

        server = ThreadingHTTPServer(('127.0.0.1', 0), ProbeHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        started = False
        try:
            control = HTTPConnection('127.0.0.1', server.server_port, timeout=5)
            try:
                control.request('GET', '/probe')
                response = control.getresponse()
                if response.status != 200 or response.read() != b"Aether network denial probe was reached":
                    raise RuntimeError('Probe server failed its positive control')
            finally:
                control.close()
            requests.clear()
            start_daemon(binary, env, str(workspace))
            started = True
            with AetherClient(env['AETHER_SOCKET']) as client:
                client.subscribe()
                agent = client.create_agent(role='coordinator', model=case['model'])
                expected_scope = 'denied' if case.get('denied') else 'allowed'
                errors = []
                if agent['scope']['network'] != expected_scope:
                    errors.append(f"Expected scope {expected_scope}, got {agent['scope']['network']}")
                session_before = None
                if case.get('resume'):
                    opposite = 'allowed' if expected_scope == 'denied' else 'denied'
                    client.request(protocol.update_agent(id=agent['id'], patch={'network': opposite}))
                    client.send(agent['id'], 'Reply ready.')
                    client.wait_for_reply(agent['id'], timeout=timeout)
                    session_before = client.get_agent(agent['id']).get('engine_session')
                    if not session_before:
                        errors.append('Warm-up did not create an engine session')
                    client.request(protocol.update_agent(id=agent['id'], patch={'network': expected_scope}))
                event_start = len(client.events_seen)
                if case.get('ask'):
                    client.request(protocol.update_agent(id=agent['id'], patch={'approval': {'require_for': ['external']}}))
                prompt = case.get('prompt', SEARCH_PROMPT)
                if case.get('denied'):
                    prompt = (f'Check that the network restriction is enforced: try fetching '
                              f'http://127.0.0.1:{server.server_port}/probe with curl inside the '
                              'existing sandbox, then report whether it was blocked. Do not bypass the sandbox.')
                client.send(agent['id'], prompt)
                try:
                    reply = client.wait_for_reply(agent['id'], timeout=timeout)
                except (TimeoutError, RuntimeError) as exc:
                    reply = ''
                    errors.append(str(exc))
                events = [event['kind'] for event in client.events_seen[event_start:]
                          if event.get('agent_id') == agent['id'] and event['kind']['type'].startswith('tool.')]
                if session_before and client.get_agent(agent['id']).get('engine_session') != session_before:
                    errors.append('Engine did not reuse the warm-up session')
                calls = [event['call']['name'] for event in events if event['type'] == 'tool.requested']
                if case.get('denied'):
                    if any(name in ('web_search', 'WebSearch', 'WebFetch') for name in calls):
                        errors.append(f'Native web call despite denied scope: {calls}')
                else:
                    if case['tool'] not in calls:
                        errors.append(f"Expected native tool {case['tool']}; got {calls}")
                    if any(tool_failed(event) for event in events):
                        errors.append('Native tool reported an error')
                    expected_text = '200' if case.get('ask') else 'doc.rust-lang.org'
                    if expected_text not in reply:
                        errors.append(f'Missing expected result {expected_text}')
                if not reply.strip():
                    errors.append('No final reply')
                result = {'case': case, 'prompt': prompt, 'scope': agent['scope']['network'],
                        'passed': not errors, 'errors': errors, 'reply': reply, 'tool_events': events,
                        'probe_requests': requests, 'probe_positive_control': True, 'resumed_session': session_before}
        finally:
            try:
                if started:
                    stop_daemon(binary, env)
            finally:
                server.shutdown()
                server.server_close()
                thread.join()
        if case.get('denied') and requests:
            errors.append(f'Denied agent reached the probe server {len(requests)} time(s)')
            result['passed'] = False
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', default='target/debug/aether')
    parser.add_argument('--out', required=True)
    parser.add_argument('--timeout', type=float, default=180)
    parser.add_argument('--case', action='append', choices=[case['name'] for case in CASES])
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    report = {'started_at': datetime.now(timezone.utc).isoformat(),
              'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(), 'results': []}
    for case in CASES:
        if args.case and case['name'] not in args.case:
            continue
        result = check(binary, case, args.timeout)
        report['results'].append(result)
        Path(args.out).write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'name': case['name'], 'passed': result['passed'], 'errors': result['errors']}), flush=True)
    raise SystemExit(0 if all(row['passed'] for row in report['results']) else 1)


if __name__ == '__main__':
    main()
