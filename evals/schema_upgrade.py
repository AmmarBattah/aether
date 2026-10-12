"""Exercise schema 8 -> 9, refusal by the old binary, and restoration of its backup."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile

from aether_client import AetherClient, init_git_workspace, isolated_env, protocol, start_daemon, stop_daemon


def snapshot(path):
    with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True) as db:
        assert db.execute('PRAGMA integrity_check').fetchone() == ('ok',)
        version = int(db.execute("SELECT value FROM meta WHERE key='schema_version'").fetchone()[0])
        tables = [row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name!='meta'")]
        rows = {name: set(db.execute('SELECT * FROM "' + name.replace('"', '""') + '"')) for name in tables}
        return version, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--old-binary', required=True)
    parser.add_argument('--new-binary', required=True)
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    old, new = str(Path(args.old_binary).resolve()), str(Path(args.new_binary).resolve())
    result = {'old_sha256': hashlib.sha256(Path(old).read_bytes()).hexdigest(),
              'new_sha256': hashlib.sha256(Path(new).read_bytes()).hexdigest()}
    with tempfile.TemporaryDirectory(prefix='aether-upgrade-') as directory:
        root = Path(directory)
        workspace = root / 'workspace'
        workspace.mkdir()
        init_git_workspace(workspace)
        env = isolated_env(str(root / 'instance'))
        database = Path(env['AETHER_DATA_DIR']) / 'aether.db'
        try:
            start_daemon(old, env, str(workspace))
            with AetherClient(env['AETHER_SOCKET']) as client:
                agents = []
                for name in ['root', 'middle', 'leaf']:
                    agent = client.request(protocol.create_agent(
                        role='fixture', name=name, objective='Preserve this upgrade fixture',
                        model={'provider': 'codex', 'model': 'gpt-5.6-terra'},
                        parent=agents[-1]['id'] if agents else None,
                        start=False, config={'paused': True, 'effort': 'low'},
                    ))['data']
                    agents.append(agent)
                client.send(agents[-1]['id'], 'Keep this undelivered correction across the upgrade.')
            stop_daemon(old, env)
            assert not Path(env['AETHER_SOCKET']).exists(), 'Old daemon did not stop'
            before_version, before = snapshot(database)
            assert before_version == 8, before_version
            backup = root / 'schema8-backup.db'
            with sqlite3.connect(database) as source, sqlite3.connect(backup) as target:
                source.backup(target)
            assert snapshot(backup) == (before_version, before)
            start_daemon(new, env, str(workspace))
            with AetherClient(env['AETHER_SOCKET']) as client:
                for agent in agents:
                    current = client.get_agent(agent['id'])
                    assert current['status'] == agent['status']
                    assert current.get('parent') == agent.get('parent')
            stop_daemon(new, env)
            assert not Path(env['AETHER_SOCKET']).exists(), 'New daemon did not stop'
            after_version, after = snapshot(database)
            assert after_version == 9, after_version
            for table, rows in before.items():
                assert rows <= after[table], f'Existing rows changed or disappeared from {table}'
            with sqlite3.connect(database) as db:
                assert db.execute("SELECT value FROM meta WHERE key='runtime.pending_cancellations'").fetchone() == ('{}',)
            refusal = subprocess.run([old, 'daemon', '--foreground'], env=env, cwd=workspace,
                                     capture_output=True, text=True, timeout=30)
            assert refusal.returncode != 0, 'Old binary accepted schema 9'
            assert '9' in refusal.stderr and '8' in refusal.stderr, refusal.stderr
            assert snapshot(database) == (after_version, after), 'Refused downgrade changed the store'
            restore_env = isolated_env(str(root / 'restored'))
            restored = Path(restore_env['AETHER_DATA_DIR']) / 'aether.db'
            shutil.copyfile(backup, restored)
            try:
                start_daemon(old, restore_env, str(workspace))
                with AetherClient(restore_env['AETHER_SOCKET']) as client:
                    for agent in agents:
                        assert client.get_agent(agent['id'])['status'] == agent['status']
            finally:
                stop_daemon(old, restore_env)
            assert not Path(restore_env['AETHER_SOCKET']).exists(), 'Restored daemon did not stop'
            restored_version, restored_rows = snapshot(restored)
            assert restored_version == 8
            for table, rows in before.items():
                assert rows <= restored_rows[table], f'Restore lost rows from {table}'
            result.update(passed=True, from_schema=before_version, to_schema=after_version,
                          preserved_rows={table: len(rows) for table, rows in before.items()},
                          downgrade_refused=True, backup_restore_verified=True)
        finally:
            stop_daemon(new, env)
    Path(args.out).write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
