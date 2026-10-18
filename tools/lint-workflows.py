#!/usr/bin/env python3
"""Run pinned actionlint with the documented concurrency.queue schema extension."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = 'v1.7.12'
TOOLCHAIN = 'go1.25.0'
SOURCE_SUM = 'h1:vQ4GeJN86C0QH+gTUQcs8McmK62OLT3kmakPMtEWYnY='


def validator():
    patch = ROOT / 'tools/actionlint-queue.patch'
    revision = hashlib.sha256(patch.read_bytes() + Path(__file__).read_bytes()).hexdigest()[:16]
    cache = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'pomotui/workflow-lint'
    cache.mkdir(parents=True, exist_ok=True)
    binary = cache / f'actionlint-{VERSION}-{TOOLCHAIN}-{revision}'
    if binary.is_file():
        return binary
    env = dict(os.environ, GOTOOLCHAIN=TOOLCHAIN, GOENV='off', GOFLAGS='',
               GOWORK='off', GOPATH=str(cache / 'go'),
               GOMODCACHE=str(cache / 'go/pkg/mod'), GOCACHE=str(cache / 'go-build'))
    print(f'Building actionlint {VERSION} with {TOOLCHAIN} and queue schema {revision}', file=sys.stderr)
    module = json.loads(subprocess.check_output(
        ['go', 'mod', 'download', '-json', f'github.com/rhysd/actionlint@{VERSION}'], env=env))
    if module.get('Sum') != SOURCE_SUM:
        raise ValueError('actionlint source checksum differs from the pinned module')
    with tempfile.TemporaryDirectory(prefix='build-', dir=cache) as tmp:
        source = Path(tmp) / 'source'
        shutil.copytree(module['Dir'], source)
        for directory, _, files in os.walk(source):
            Path(directory).chmod(0o755)
            for name in files:
                (Path(directory) / name).chmod(0o644)
        subprocess.run(['patch', '--batch', '--fuzz=0', '-p1', '-i', str(patch)], cwd=source, check=True)
        output = Path(tmp) / 'actionlint'
        subprocess.run(['go', 'build', '-mod=readonly', '-buildvcs=false', '-trimpath', '-o', str(output),
                        './cmd/actionlint'], cwd=source, env=env, check=True)
        os.replace(output, binary)
    return binary


def main():
    try:
        binary = validator()
        return subprocess.run([str(binary), '-shellcheck=', '-pyflakes=', *sys.argv[1:]], cwd=ROOT).returncode
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f'Workflow validation failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
