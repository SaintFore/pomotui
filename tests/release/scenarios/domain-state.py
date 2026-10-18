#!/usr/bin/env python3
"""Real package upgrades observed through protocol v6; run only in disposable Arch."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import sqlite3
import subprocess
import sys
import time
import uuid

old, candidate, directory, user, uid = sys.argv[1:]
out = Path(directory) / 'domain-state'
out.mkdir(parents=True, exist_ok=True)
home = Path('/home') / user
data = home / '.local/share/pomotui'
config = home / '.config/pomotui/config.toml'
env = ['env', f'XDG_RUNTIME_DIR=/run/user/{uid}', f'DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus']

def run(*args, check=True):
    result = subprocess.run(args, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if check and result.returncode:
        print(result.stdout, flush=True)
        result.check_returncode()
    return result

def manager(*args, check=True):
    return run('runuser', '-u', user, '--', *env, 'systemctl', '--user', *args, check=check)

def request(command='status', **fields):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(8)
        connection.connect(f'/run/user/{uid}/pomotui/pomotui.sock')
        connection.sendall((json.dumps({'version': 6, 'idempotency_key': str(uuid.uuid4()), 'command': {'command': command, **fields}}) + '\n').encode())
        response = json.loads(connection.makefile().readline())
    assert response['result'] != 'error', response
    return response.get('snapshot', response.get('value'))

def record(name, value):
    (out / f'{name}.json').write_text(json.dumps(value, indent=2) + '\n')

def fresh(name):
    print(f'SCENARIO {name}', flush=True)
    manager('disable', '--now', 'pomotui.socket', check=False)
    manager('stop', 'pomotui.service', check=False)
    run('pacman', '-U', '--noconfirm', old)
    shutil.rmtree(data, ignore_errors=True)
    shutil.rmtree(config.parent, ignore_errors=True)
    config.parent.mkdir(parents=True)
    config.write_text('focus_minutes = 13\nshort_break_minutes = 3\nlong_break_minutes = 9\nrounds_per_cycle = 3\nreservation_minutes = 7\nreminder_enabled = false\nvolume = 23\nlanguage = "en"\n')
    run('chown', '-R', f'{user}:{user}', str(home))
    manager('daemon-reload')
    manager('reset-failed', 'pomotui.service', check=False)
    manager('enable', '--now', 'pomotui.socket')
    request()

def upgrade(name):
    before = request()
    record(name + '-before', before)
    pid = manager('show', 'pomotui.service', '-p', 'MainPID', '--value').stdout.strip()
    settings = config.read_bytes()
    began = time.monotonic()
    result = run('pacman', '-U', '--noconfirm', candidate)
    (out / f'{name}-pacman.log').write_text(result.stdout)
    new_pid = manager('show', 'pomotui.service', '-p', 'MainPID', '--value').stdout.strip()
    assert new_pid != pid and int(new_pid) > 0, (pid, new_pid)
    assert Path(f'/proc/{new_pid}/exe').read_bytes() == Path('/usr/bin/pomotui-service').read_bytes()
    assert manager('show', 'pomotui.service', '-p', 'FragmentPath', '--value').stdout.strip() == '/usr/lib/systemd/user/pomotui.service'
    after = request()
    record(name + '-after', after)
    assert after['durable_health']['state'] == 'healthy', after
    assert config.read_bytes() == settings
    assert after['reservation_minutes'] == 7 and after['rounds_per_cycle'] == 3
    record(name + '-identity', {'old_pid': int(pid), 'candidate_pid': int(new_pid), 'candidate_service_sha256': hashlib.sha256(Path('/usr/bin/pomotui-service').read_bytes()).hexdigest(), 'upgrade_elapsed_seconds': time.monotonic() - began})
    for field in ['action_chain', 'pending_review', 'recent_chain_links', 'recent_ended_chains', 'reward_milestones', 'current_chain_rewards', 'reward_debt']:
        assert after[field] == before[field], (name, field, before[field], after[field])
    return before, after, time.monotonic() - began

def start(task):
    return request('start', kind='focus', task_id=task)

def create_task():
    request('task_create', title='Upgrade acceptance work')
    return next(task['id'] for task in request()['tasks'] if task['title'] == 'Upgrade acceptance work')

fresh('running')
task = create_task()
start(task)
time.sleep(2)
before, after, downtime = upgrade('running')
assert after['state'] == 'running'
for field in ['kind', 'planned_seconds', 'current_task_id', 'current_task']:
    assert after[field] == before[field], field
assert after['planned_seconds'] == 780
assert -1 <= before['remaining_seconds'] - after['remaining_seconds'] <= downtime + 3
remaining = after['remaining_seconds']
time.sleep(2)
assert request()['remaining_seconds'] < remaining
# The public live snapshot has no Session ID. Ending the continued session must
# yield one attributed History record, whose ID is retained by Pending Review.
ended = request('stop_review')
assert len(ended['recent_history']) == 1
assert ended['pending_review']['session_id'] == ended['recent_history'][0]['id']
assert ended['pending_review']['task_id'] == task
record('running-ended', ended)
rest = request('start', kind='short_break', task_id=None)
assert rest['planned_seconds'] == 180

fresh('paused')
task = create_task()
start(task)
time.sleep(2)
request('pause')
before, after, _ = upgrade('paused')
assert after['state'] == 'paused'
assert after['remaining_seconds'] == before['remaining_seconds']
assert after['current_task_id'] == task and after['planned_seconds'] == 780
time.sleep(2)
assert request()['remaining_seconds'] == after['remaining_seconds']

fresh('pending-review-and-durable-state')
task = create_task()
request('reward_create', name='Acceptance reward', threshold=1, budget=42)
start(task)
time.sleep(1)
request('stop_review')
request('review_success', reflection='Preserved successful work')
unlock = request()['current_chain_rewards'][0]['id']
request('reward_claim', unlock_id=unlock)
start(task)
time.sleep(1)
request('stop_review')
before, after, _ = upgrade('pending-review-and-durable-state')
for field in ['tasks', 'recent_history']:
    assert after[field] == before[field], field
assert after['pending_review'] is not None
assert after['action_chain']['length'] == 1
assert after['current_chain_rewards'][0]['state'] == 'claimed'
assert after['recent_chain_links'][0]['reflection'] == 'Preserved successful work'
assert len(after['recent_history']) == 2

fresh('reservation')
task = create_task()
request('reserve', minutes=1, task_id=task, use_current_task=False)
before, after, downtime = upgrade('reservation')
a, b = before['reservation'], after['reservation']
for field in ['id', 'task_id', 'task_title', 'duration_seconds']:
    assert a[field] == b[field], field
assert b['task_id'] == task and b['duration_seconds'] == 60
assert -1 <= a['remaining_seconds'] - b['remaining_seconds'] <= downtime + 3
# Observe real expiry, then upgrade the old package's pending judgment as well.
time.sleep(b['remaining_seconds'] + 1)
expired = request()
assert expired['reservation']['state'] == 'awaiting_review'
assert expired['action_chain'] == before['action_chain']
assert expired['recent_ended_chains'] == []
record('reservation-expired', expired)
fresh('reservation-pending-judgment')
task = create_task()
request('reserve', minutes=1, task_id=task, use_current_task=False)
time.sleep(61)
before, after, _ = upgrade('reservation-pending-judgment')
assert before['reservation']['state'] == 'awaiting_review'
assert after['reservation'] == before['reservation']
assert after['recent_ended_chains'] == []
print('PASS: running, paused, pending review, durable state, reservation countdown/expiry', flush=True)

for version in ['v0.1.0', 'v0.2.0']:
    fresh('migration-' + version)
    manager('stop', 'pomotui.socket', 'pomotui.service')
    shutil.rmtree(data)
    data.mkdir()
    fixture = Path('/acceptance/scenarios/fixtures') / (version + '-production.sqlite3')
    shutil.copyfile(fixture, data / 'pomotui.sqlite3')
    run('chown', '-R', f'{user}:{user}', str(data))
    manager('start', 'pomotui.socket')
    migrated = request()
    assert migrated['tasks'] and migrated['recent_history'] and migrated['pending_review'], migrated
    backups = list(data.glob('*backup-schema-migration-*'))
    assert len(backups) == 1, backups
    backup_bytes = backups[0].read_bytes()
    before, after, _ = upgrade('migration-' + version)
    for field in ['tasks', 'recent_history', 'pending_review']:
        assert after[field] == before[field], (version, field)
    assert backups[0].read_bytes() == backup_bytes
print('PASS: released v0.1.0/v0.2.0 migrations and original backup preserved', flush=True)

# Malformed/unsupported input is deliberately injected at the on-disk support
# boundary; observations are failed public startup, journal diagnosis and bytes.
for name in ['corrupt', 'unsupported-schema', 'unsupported-payload']:
    fresh(name)
    manager('stop', 'pomotui.socket', 'pomotui.service')
    database = data / 'pomotui.sqlite3'
    for suffix in ['-wal', '-shm']:
        Path(str(database) + suffix).unlink(missing_ok=True)
    if name == 'corrupt':
        database.write_bytes(b'Pomotui damaged database acceptance sentinel\n')
    elif name == 'unsupported-schema':
        with sqlite3.connect(database) as connection:
            connection.execute('PRAGMA user_version = 2147483647')
    else:
        with sqlite3.connect(database) as connection:
            payload = json.loads(connection.execute('SELECT payload FROM current_session').fetchone()[0])
            payload['data_format_version'] = 65535
            connection.execute('UPDATE current_session SET payload = ?', (json.dumps(payload),))
    original = database.read_bytes()
    run('pacman', '-U', '--noconfirm', candidate)
    manager('reset-failed', 'pomotui.service', check=False)
    result = manager('start', 'pomotui.service', check=False)
    assert result.returncode != 0, name
    journal = manager('status', 'pomotui.service', '--no-pager', check=False).stdout
    journal += run('journalctl', f'_UID={uid}', '--no-pager').stdout
    (out / f'{name}-diagnosis.log').write_text(journal)
    expected = {'corrupt': 'not a database', 'unsupported-schema': 'newer', 'unsupported-payload': 'unsupported'}[name]
    assert expected in journal.lower(), (name, journal)
    assert database.read_bytes() == original, name
    record(name + '-preservation', {'sha256': hashlib.sha256(original).hexdigest(), 'startup_rejected': True})
print('PASS: damaged and unsupported databases rejected without changing original data', flush=True)
