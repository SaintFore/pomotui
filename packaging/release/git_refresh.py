#!/usr/bin/env python3
"""Select relevant main changes and retain a source-identified Git candidate."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from aur import AurChannel

RELEVANT = ('crates/', 'packaging/', 'assets/', 'completions/', 'scripts/', 'tests/release/')
ROOT_FILES = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'build.rs'}


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args], text=True).strip()


def select(repo, source, before=None):
    commit = git(repo, 'rev-parse', f'{source}^{{commit}}')
    parent = before or git(repo, 'rev-parse', f'{commit}^')
    initial = parent == '0' * 40
    if not initial and subprocess.run(['git', '-C', str(repo), 'merge-base', '--is-ancestor', parent, commit], capture_output=True).returncode:
        raise ValueError('Git refresh baseline is not an ancestor of candidate source')
    paths = (git(repo, 'ls-tree', '-r', '--name-only', commit) if initial else
             git(repo, 'diff', '--name-only', parent, commit)).splitlines()
    # Snapshot metadata is generated in the candidate artifact, never committed
    # back to main. Ignore those paths if an older automation touches them.
    generated = {'packaging/aur/.SRCINFO', 'packaging/aur/pomotui-git/.SRCINFO'}
    relevant = [path for path in paths if path not in generated and
                (path in ROOT_FILES or path.startswith(RELEVANT) or path.startswith('.github/workflows/'))]
    return dict(selected=bool(relevant), source_commit=commit, intent='git', changed=relevant,
                reason='source or packaging changed' if relevant else 'no source or packaging changes')


def publish(args):
    manifest = json.loads((args.candidate / 'candidate.json').read_text())
    if manifest['source_commit'] != args.source:
        raise ValueError('Git publication source differs from verified workflow source')
    subprocess.run([sys.executable, str(Path(__file__).with_name('release.py')), 'verify-gates',
        '--candidate', str(args.candidate), '--acceptance', str(args.acceptance),
        '--checks', str(args.checks)], check=True, stdout=subprocess.DEVNULL)
    return AurChannel(args.candidate, 'aur-git', args.remote, args.rpc_url,
                      acceptance=args.acceptance, discovery_attempts=args.discovery_attempts,
                      discovery_delay=args.discovery_delay).publish()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='action', required=True)
    selection = commands.add_parser('select')
    selection.add_argument('--repo', type=Path, default=Path('.'))
    identity = selection.add_mutually_exclusive_group(required=True)
    identity.add_argument('--source')
    identity.add_argument('--event', type=Path)
    selection.add_argument('--before')
    publication = commands.add_parser('publish')
    publication.add_argument('--source', required=True)
    publication.add_argument('--candidate', type=Path, required=True)
    publication.add_argument('--acceptance', type=Path, required=True)
    publication.add_argument('--checks', type=Path, required=True)
    publication.add_argument('--remote')
    publication.add_argument('--rpc-url')
    publication.add_argument('--discovery-attempts', type=int, default=6)
    publication.add_argument('--discovery-delay', type=float, default=10)
    args = parser.parse_args()
    if args.action == 'select':
        if args.event:
            event = json.loads(args.event.read_text())
            if event.get('ref') != 'refs/heads/main' or event.get('deleted') or event.get('after') == '0' * 40:
                raise ValueError('Git refresh requires a non-deleted main push')
            result = select(args.repo, event['after'], event['before'])
        else:
            result = select(args.repo, args.source, args.before)
    else:
        result = publish(args)
    print(json.dumps(result))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error))
