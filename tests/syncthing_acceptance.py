#!/usr/bin/env python3
"""Opt-in real transport acceptance; never opens a production profile or database."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import socket
import tempfile
import time
import uuid
from syncthing_pair import SyncthingPair


def encode(document):
    records = sorted(document['records'], key=lambda r: r['id'])
    document['records'] = records
    raw = json.dumps([document['beginning'], records], separators=(',', ':'), ensure_ascii=False).encode()
    document['integrity'] = dict(record_count=len(records), records_sha256=hashlib.sha256(raw).hexdigest())
    return json.dumps(document, ensure_ascii=False)


def record(entity, stamp, kind, data):
    return dict(id=str(uuid.uuid4()), entity_id=identity(entity), mutation_time=stamp,
                payload=dict(type=kind, data=data))


def identity(n):
    return f'00000000-0000-0000-0000-{n:012x}'


def review(n, stamp, failed=False):
    return [record(n + 100, stamp, 'session_ended', dict(ended_at=stamp, kind='focus', outcome='stopped', planned_seconds=60, actual_seconds=60, task_entity_id=None, task_title=None)),
            record(n, stamp, 'session_reviewed', dict(session_entity_id=identity(n + 100), judgment='failed' if failed else 'successful', task_entity_id=identity(50), task_kind='system_void', task_title='Void', actual_seconds=60, reflection='late break' if failed else None, chain_entry_title='work'))]


class App:
    def __init__(self, root, binaries, rounds):
        self.root, self.binaries = root, binaries
        self.env = os.environ.copy()
        self.env.update(POMOTUI_SOCKET=str(root/'runtime/pomotui.sock'), XDG_DATA_HOME=str(root/'data'), XDG_CONFIG_HOME=str(root/'config'))
        (root/'runtime').mkdir(parents=True)
        (root/'config/pomotui').mkdir(parents=True)
        (root/'config/pomotui/config.toml').write_text(f'focus_minutes = 1\nrounds_per_cycle = {rounds}\nreminder_enabled = false\n')
        self.log = open(root/'service.log', 'w')
        self.start()

    def start(self):
        self.process = subprocess.Popen([str(self.binaries/'pomotui-service')], env=self.env, stdout=self.log, stderr=self.log)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            try:
                self.cli('status')
                return
            except (subprocess.CalledProcessError, ValueError):
                time.sleep(.05)
        self.close()
        raise AssertionError('Timer Service startup failed')

    def cli(self, *args):
        out = subprocess.check_output([str(self.binaries/'pomotui'), '--json', *map(str, args)], env=self.env, stderr=subprocess.PIPE)
        response = json.loads(out)
        if response.get('result') == 'error':
            raise AssertionError(response)
        return response.get('snapshot', response.get('value', response))

    def protocol(self, command, **fields):
        # Same public protocol used by the TUI for archive deletion.
        with socket.socket(socket.AF_UNIX) as connection:
            connection.connect(self.env['POMOTUI_SOCKET'])
            request = dict(version=5, idempotency_key=str(uuid.uuid4()), command=dict(command=command, **fields))
            connection.sendall(json.dumps(request).encode() + b'\n')
            response = json.loads(connection.makefile().readline())
            assert response['result'] != 'error', response
            return response.get('snapshot', response.get('value', response))

    def sync(self):
        self.cli('sync', 'now')
        time.sleep(.1)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            status = self.cli('sync', 'status')
            if not status['in_progress']:
                assert status['last_error'] is None, status
                return status
            time.sleep(.05)
        raise AssertionError('Timer Service synchronization timed out')

    def stop(self):
        self.process.terminate()
        self.process.wait(timeout=10)

    def close(self):
        self.stop()
        self.log.close()


def run(root, binaries, old_binaries=None):
    pair = SyncthingPair(root/'transport')
    apps = []
    passed = []
    def check(label):
        passed.append(label)
        print('PASS:', label, flush=True)
    try:
        pair.start()
        a, b = (p['replica']/'pomotui.sync' for p in pair.peers)
        apps.append(App(root/'app-a', binaries, 4))
        apps.append(App(root/'app-b', binaries, 7))
        first, second = apps
        first.cli('sync', 'enable', a)
        first.cli('sync', 'rebuild')
        first.sync()
        pair.scan()
        pair.wait(lambda: b.exists() and b.read_bytes() == a.read_bytes())
        second.cli('sync', 'enable', b)
        second.sync()
        pair.pause(True)
        for app, title in zip(apps, ['offline A', 'offline B']):
            app.cli('task', 'create', title)
            app.cli('start', 'focus')
            app.cli('stop', '--review')
            app.cli('review', 'success', '--void', title)
            app.cli('start', 'focus')
            app.sync()
        stale = a.read_bytes()
        # Prevent the app worker consuming a real conflict before observing it.
        for app in apps:
            app.cli('sync', 'disable')
        pair.scan()
        pair.pause(False)
        pair.wait(lambda: any(list(p['replica'].glob('*sync-conflict*')) for p in pair.peers))
        check('real disconnected divergent writes generated a Syncthing conflict sibling')
        for app, path in zip(apps, [a, b]):
            app.cli('sync', 'enable', path)
        def converge():
            for app in apps:
                app.sync()
            pair.scan()
            return a.read_bytes() == b.read_bytes() and len(first.cli('task', 'list')) == 3 and len(second.cli('task', 'list')) == 3
        pair.wait(converge)
        assert len(first.cli('history')) == len(second.cli('history')) == 2
        pair.wait(lambda: not any(list(p['replica'].glob('*sync-conflict*')) for p in pair.peers))
        assert [app.cli('status')['state'] for app in apps] == ['running', 'running']
        assert [app.cli('status')['rounds_per_cycle'] for app in apps] == [4, 7]
        check('complete union and real conflict cleanup; independent live timers and local settings')
        pair.pause(True)
        before = a.stat()
        for _ in range(3):
            first.sync()
        after = a.stat()
        assert (before.st_ino, before.st_mtime_ns) == (after.st_ino, after.st_mtime_ns)
        subset = a.with_name('pomotui.sync-conflict-20261005-010101-TEST.sync')
        subset.write_bytes(stale)
        first.sync()
        assert not subset.exists()
        assert len(first.cli('task', 'list')) == 3
        a.write_bytes(stale)
        first.sync()
        assert len(first.cli('task', 'list')) == 3
        assert len(json.loads(a.read_text())['records']) > len(json.loads(stale)['records'])
        check('idle inode/mtime stable; absorbed subset cleaned; stale overwrite repairs retained union')
        invalid = a.with_name('pomotui.sync-conflict-20261005-010102-TEST.sync')
        invalid.write_text('{invalid')
        first.cli('sync', 'now')
        pair.wait(lambda: first.cli('sync', 'status')['last_error'] is not None)
        assert invalid.exists()
        first.cli('pause')
        first.cli('resume')
        invalid.unlink()
        first.sync()
        contradiction = json.loads(a.read_text())
        task = next(r for r in contradiction['records'] if r['payload']['type'] == 'task_version')
        task['payload']['data']['title'] = 'contradiction under unchanged Record ID'
        invalid.write_text(encode(contradiction))
        first.cli('sync', 'now')
        pair.wait(lambda: first.cli('sync', 'status')['last_error'] is not None)
        assert invalid.exists()
        print('DIAGNOSTIC:', first.cli('sync', 'status')['last_error'], flush=True)
        invalid.unlink()
        first.sync()
        for fault in ('checksum', 'unsupported-version'):
            damaged = json.loads(a.read_text())
            if fault == 'checksum':
                damaged['integrity']['records_sha256'] = '0' * 64
            else:
                damaged['version'] = 65535
            invalid.write_text(json.dumps(damaged))
            first.cli('sync', 'now')
            pair.wait(lambda: first.cli('sync', 'status')['last_error'] is not None)
            assert invalid.exists()
            invalid.unlink()
            first.sync()
        invalid.symlink_to(a.name)
        first.cli('sync', 'now')
        pair.wait(lambda: first.cli('sync', 'status')['last_error'] is not None)
        assert invalid.is_symlink()
        invalid.unlink()
        first.sync()
        check('invalid JSON/checksum/version, same-ID contradiction and unsafe symlink retained with diagnosis; timer usable')
        # A public file import supplies controlled event times; the real service projects debt.
        for app in apps:
            app.cli('fresh-start', '--confirm')
        pair.pause(False)
        def converge_equal():
            left, right = first.sync(), second.sync()
            pair.scan()
            return (a.read_bytes() == b.read_bytes() and
                    left['local_record_count'] == right['local_record_count'] ==
                    len(json.loads(a.read_text())['records']))
        pair.wait(converge_equal)
        first.sync()
        second.sync()
        pair.pause(True)
        doc = json.loads(a.read_text())
        beginning = doc['beginning']
        def inject(path, additions):
            document = json.loads(path.read_text())
            for rec in additions:
                rec['beginning'] = beginning
                # Rust serialization places non-genesis beginning before ID.
                rec = dict(beginning=rec.pop('beginning'), **rec)
                document['records'].append(rec)
            path.write_text(encode(document))
        inject(a, [r for n in range(1, 8) for r in review(n, n*10)])
        first.sync()
        first.cli('reward', 'create', 7, 'Coffee')
        first.sync()
        snapshot = first.cli('status')
        first.cli('reward', 'claim', snapshot['current_chain_rewards'][0]['id'])
        first.sync()
        pair.pause(False)
        pair.wait(converge_equal)
        pair.pause(True)
        inject(b, review(20, 65, True))
        second.sync()
        assert second.cli('status')['reward_debt'][0]['outstanding'] == 6
        pair.pause(False)
        pair.wait(converge_equal)
        assert first.cli('status')['reward_debt'][0]['outstanding'] == 6
        check('late actual imported review revises claimed threshold-seven reward to debt six on both devices')
        pair.pause(True)
        inject(a, [r for n in range(8, 10) for r in review(n, n*10)])
        inject(b, [r for n in range(10, 12) for r in review(n, n*10)])
        for app in apps:
            app.sync()
            app.cli('sync', 'disable')
        pair.scan()
        pair.pause(False)
        pair.wait(lambda: any(list(p['replica'].glob('*sync-conflict*')) for p in pair.peers))
        for app, path in zip(apps, [a, b]):
            app.cli('sync', 'enable', path)
        pair.wait(converge_equal)
        assert first.cli('status')['reward_debt'][0]['outstanding'] == 2
        assert first.cli('status')['reward_debt'][0]['repaid'] == 4
        pair.pause(True)
        inject(a, [r for n in range(21, 24) for r in review(n, 45+n)])
        first.sync()
        debt = first.cli('status')['reward_debt'][0]
        assert (debt['outstanding'], debt['excess_credit']) == (0, 1), debt
        for chain in first.cli('status')['recent_ended_chains']:
            first.protocol('ended_chain_delete', id=chain['id'])
        assert not first.cli('status')['recent_ended_chains']
        assert first.cli('status')['reward_debt'][0] == debt
        first.cli('reward', 'delete', first.cli('status')['reward_milestones'][0]['id'])
        first.sync()
        assert first.cli('status')['reward_debt'][0] == debt
        pair.pause(False)
        pair.wait(converge_equal)
        assert second.cli('status')['reward_debt'][0] == debt
        for app in apps:
            app.stop()
            app.start()
            app.sync()
            assert app.cli('status')['reward_debt'][0] == debt
        check('competing offline repayment credits four once; correction retains excess credit; history/reward deletion and restart preserve accounting')
        pair.pause(True)
        old = b.read_bytes()
        first.cli('fresh-start', '--confirm')
        first.cli('task', 'create', 'new beginning')
        first.cli('start', 'focus')
        first.cli('stop', '--review')
        first.cli('review', 'success', '--void', 'new beginning review')
        first.sync()
        second.cli('task', 'create', 'retired offline work')
        second.cli('start', 'focus')
        second.sync()
        pair.pause(False)
        pair.wait(converge_equal)
        assert second.cli('status')['state'] == 'pending'
        assert not second.cli('status')['reward_debt']
        assert sorted(t['title'] for t in second.cli('task', 'list')) == ['Void', 'new beginning']
        notice = second.cli('sync', 'status')['warning']
        assert notice and 'Fresh Start' in notice, notice
        pair.pause(True)
        b.write_bytes(old)
        subset = b.with_name('pomotui.sync-conflict-20261005-010103-TEST.sync')
        subset.write_bytes(old)
        second.sync()
        second.stop()
        second.start()
        second.sync()
        assert len(second.cli('task', 'list')) == 2
        assert not second.cli('status')['reward_debt']
        check('offline old work retired with notice; stale main/copy and restart cannot resurrect debt or history')
        third = App(root/'app-c', binaries, 9)
        apps.append(third)
        copy = root/'copied.sync'
        copy.write_bytes(a.read_bytes())
        third.cli('sync', 'enable', copy)
        third.sync()
        assert sorted(t['title'] for t in third.cli('task', 'list')) == ['Void', 'new beginning']
        assert third.cli('status')['rounds_per_cycle'] == 9
        copy.write_bytes(old)
        third.sync()
        assert len(third.cli('task', 'list')) == 2
        check('one copied main carries current activity and reset beginning to fresh Device; local settings retained')
        if old_binaries:
            legacy = App(root/'legacy-app', old_binaries, 5)
            apps.append(legacy)
            legacy_copy = root/'legacy-new-format.sync'
            legacy_copy.write_bytes(a.read_bytes())
            original = legacy_copy.read_bytes()
            legacy.cli('task', 'create', 'local legacy Task')
            before = legacy.cli('task', 'list')
            legacy.cli('sync', 'enable', legacy_copy)
            legacy.cli('sync', 'now')
            pair.wait(lambda: legacy.cli('sync', 'status')['last_error'] is not None)
            error = legacy.cli('sync', 'status')['last_error']
            assert 'unsupported' in error and '7' in error, error
            assert legacy_copy.read_bytes() == original
            assert legacy.cli('task', 'list') == before
            legacy.cli('start', 'focus')
            print('LEGACY DIAGNOSTIC:', error, flush=True)
            check('actual baseline old binary rejects format 7 without file or business-state mutation; timer stays usable')
        return passed
    finally:
        for app in apps:
            app.close()
        pair.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binaries', type=Path, default=Path('target/debug'))
    parser.add_argument('--old-binaries', type=Path, help='optional archived old service/CLI build for actual format refusal')
    parser.add_argument('--keep', type=Path, help='new empty scratch directory to retain logs')
    args = parser.parse_args()
    print(subprocess.check_output(['syncthing', '--version'], text=True).strip(), flush=True)
    if args.keep:
        args.keep.mkdir(parents=True, exist_ok=False)
        run(args.keep.resolve(), args.binaries.resolve(), args.old_binaries.resolve() if args.old_binaries else None)
    else:
        with tempfile.TemporaryDirectory(prefix='pomotui-real-syncthing-') as scratch:
            run(Path(scratch), args.binaries.resolve(), args.old_binaries.resolve() if args.old_binaries else None)
