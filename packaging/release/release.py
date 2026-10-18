#!/usr/bin/env python3
"""Coordinate an approved stable release without rewriting public identities."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def select(args):
    event = json.loads(args.event.read_text())
    pr = event.get('pull_request', {})
    result = {'intent': 'git', 'reason': 'ordinary merge is not stable release intent'}
    if re.fullmatch(r'release: v[0-9]+\.[0-9]+\.[0-9]+', pr.get('title', '')):
        raise ValueError('stable intent requires authoritative approved release PR verification')
    args.output.write_text(json.dumps(result, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    p = commands.add_parser('select')
    p.add_argument('--event', type=Path, required=True)
    p.add_argument('--repo', type=Path, default=Path('.'))
    p.add_argument('--output', type=Path, required=True)
    p.set_defaults(func=select)
    args = parser.parse_args()
    try:
        args.func(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Paused: {error}\n')


if __name__ == '__main__':
    main()
