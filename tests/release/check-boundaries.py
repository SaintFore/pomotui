#!/usr/bin/env python3
"""Run fast release boundary tests, optionally at another isolated workspace version."""
import argparse
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def change_version(repo, version):
    manifest = repo / 'Cargo.toml'
    workspace = tomllib.loads(manifest.read_text())['workspace']
    names = {tomllib.loads((repo / member / 'Cargo.toml').read_text())['package']['name']
             for member in workspace['members']}
    manifest.write_text(re.sub(r'(?m)^(version\s*=\s*)"[^"]+"',
                               lambda match: match[1] + f'"{version}"', manifest.read_text(), count=1))
    if tomllib.loads(manifest.read_text())['workspace']['package']['version'] != version:
        raise ValueError('could not select the isolated workspace version')
    lock = repo / 'Cargo.lock'
    blocks = re.split(r'(?m)(?=^\[\[package\]\]\s*$)', lock.read_text())
    for index, block in enumerate(blocks):
        if not block.startswith('[[package]]'):
            continue
        package = tomllib.loads(block)['package'][0]
        if package['name'] in names and 'source' not in package:
            blocks[index] = re.sub(r'(?m)^version = "[^"]+"', f'version = "{version}"', block, count=1)
    lock.write_text(''.join(blocks))


def check(repo):
    print(f'Release boundary checks in {repo}', flush=True)
    return subprocess.run([sys.executable, '-m', 'unittest', 'discover',
                           '-s', 'tests/release', '-p', '*_test.py'], cwd=repo).returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=ROOT)
    parser.add_argument('--version', help='stable semantic version for a disposable workspace')
    args = parser.parse_args()
    if not args.version:
        return check(args.repo.resolve())
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', args.version):
        parser.error('--version requires a stable semantic version')
    repo = args.repo.resolve()
    files = subprocess.check_output(['git', '-C', str(repo), 'ls-files', '-z',
                                     '--cached', '--others', '--exclude-standard']).decode().split('\0')
    with tempfile.TemporaryDirectory(prefix='pomotui-boundaries-') as tmp:
        isolated = Path(tmp)
        for name in filter(None, files):
            source, dest = repo / name, isolated / name
            if source.is_file() or source.is_symlink():
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, dest, follow_symlinks=False)
        change_version(isolated, args.version)
        return check(isolated)


if __name__ == '__main__':
    raise SystemExit(main())
