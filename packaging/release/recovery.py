"""Durable publication evidence and bounded reconciliation after uncertain writes."""
import contextlib
import fcntl
import hashlib
import json
import os
import subprocess
from pathlib import Path
import time


def complete(stage):
    return stage in ('discoverable','superseded-discoverable')


def remote_verified(stage):
    return complete(stage) or stage in ('remote-verified','pending-indexing')


class TransientError(OSError):
    """Retry the whole idempotent operation, rereading authoritative remotes."""


def atomic(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + '.tmp')
    with temporary.open('w') as output:
        output.write(json.dumps(value, indent=2) + '\n')
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary, path)


class Journal:
    def __init__(self, path, directory):
        self.path = Path(path)
        manifest = json.loads((directory / 'candidate.json').read_text())
        identity = hashlib.sha256((directory / 'candidate.json').read_bytes()).hexdigest()
        self.value = json.loads(self.path.read_text()) if self.path.exists() else {
            'schema': 1, 'candidate_sha256': identity,
            'source_commit': manifest['source_commit'], 'version': manifest['version'],
            'channels': {}, 'events': [], 'attempts': []}
        if self.value.get('candidate_sha256') != identity:
            raise ValueError('publication journal identifies another immutable candidate; restore original artifacts')
        self.value.setdefault('attempts', [])

    def save(self):
        atomic(self.path, self.value)

    def event(self, value):
        self.value['events'].append(value)
        self.value['channels'][value['channel']] = value
        self.save()

    def execute(self, operation, attempts=3, delay=2):
        self.value['aggregate'] = 'in-progress'
        self.value.pop('diagnosis', None)
        for number in range(max(1, min(attempts, 5))):
            attempt = {'number': len(self.value['attempts']) + 1, 'stage': 'started'}
            self.value['attempts'].append(attempt)
            self.save()
            try:
                result = operation()
                attempt['stage'] = 'finished'
                self.save()
                return result
            except (ValueError, OSError, subprocess.SubprocessError) as error:
                attempt.update(stage='failed', diagnosis=str(error))
                self.value['diagnosis'] = str(error)
                transient = isinstance(error, (TransientError, TimeoutError, ConnectionError))
                if not transient or number + 1 >= max(1, min(attempts, 5)):
                    if self.value['aggregate'] == 'in-progress':
                        self.value['aggregate'] = 'retry-exhausted' if transient else 'paused'
                    self.save()
                    raise
                self.save()
                time.sleep(max(0, min(delay, 30)))


@contextlib.contextmanager
def locked(path):
    """Local duplicate attempts serialize; remote writes still require FF/CAS."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.with_name(path.name + '.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield
