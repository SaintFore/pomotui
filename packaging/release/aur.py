#!/usr/bin/env python3
"""Publish a verified candidate to AUR; never edit version/source metadata here.

Production credentials are usable only inside the gated publication workflow.
Local Git/HTTP endpoints support the same channel boundary without public writes.
"""
import argparse
import contextlib
import hashlib
import io
import json
import os
import re
from pathlib import Path
import shlex
import subprocess
import tempfile
import time
import urllib.parse
import urllib.request

import candidate

FILES = ('PKGBUILD', '.SRCINFO', 'pomotui.install')
PACKAGES = {'aur-git': 'pomotui-git', 'aur-stable': 'pomotui'}


def run(*args, cwd=None, env=None):
    result = subprocess.run(args, cwd=cwd, env=env, text=True, capture_output=True, timeout=60)
    if result.returncode:
        raise ValueError(f'{args[0]} failed: {result.stderr.strip() or result.stdout.strip()}')
    return result.stdout.strip()


class AurChannel:
    def __init__(self, directory, channel, remote=None, rpc_url=None, known_hosts=None,
                 acceptance=None, discovery_attempts=6, discovery_delay=10):
        self.directory = Path(directory).resolve()
        self.channel = channel
        self.package = PACKAGES[channel]
        self.remote = remote or f'ssh://aur@aur.archlinux.org/{self.package}.git'
        self.rpc_url = rpc_url or 'https://aur.archlinux.org/rpc/v5/info'
        self.known_hosts = Path(known_hosts or Path(__file__).parents[1] / 'aur/known_hosts').resolve()
        self.acceptance = Path(acceptance) if acceptance else None
        self.discovery_attempts = max(1, min(discovery_attempts, 30))
        self.discovery_delay = max(0, min(discovery_delay, 60))
        # Endpoint overrides are explicitly local only: never allow credentials to
        # escape to an arbitrary SSH host or replace production TLS with HTTP.
        self.local = '://' not in self.remote or self.remote.startswith('file://')
        if not self.local and self.remote != f'ssh://aur@aur.archlinux.org/{self.package}.git':
            raise ValueError('unsupported AUR SSH destination')
        rpc = urllib.parse.urlparse(self.rpc_url)
        if rpc.scheme != 'https' and not (self.local and rpc.hostname in ('127.0.0.1', 'localhost')):
            raise ValueError('AUR discovery requires HTTPS (HTTP allowed only for isolated local endpoints)')
        self.env = os.environ.copy()
        self.env.pop('GIT_SSH_COMMAND', None)
        self.env.pop('GIT_SSH', None)
        self.ssh = ['ssh', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
                    '-o', f'UserKnownHostsFile={self.known_hosts}', '-o', 'GlobalKnownHostsFile=/dev/null',
                    '-o', 'ConnectTimeout=20', '-o', 'IdentitiesOnly=yes']
        key = os.environ.get('AUR_SSH_KEY_FILE')
        if key:
            self.ssh += ['-i', key]
        self.env['GIT_SSH_COMMAND'] = shlex.join(self.ssh)
        with contextlib.redirect_stdout(io.StringIO()):
            candidate.verify(argparse.Namespace(output=self.directory, require_built=True))
        self.manifest = json.loads((self.directory / 'candidate.json').read_text())
        if channel not in self.manifest['required_channels']:
            raise ValueError(f'candidate does not require {channel}')
        info = (self.directory / channel / '.SRCINFO').read_text()
        fields = {}
        for line in info.splitlines():
            if ' = ' in line:
                name, value = line.strip().split(' = ', 1)
                fields.setdefault(name, value)
        expected_version = self.manifest['git_version'] if channel == 'aur-git' else self.manifest['version']
        if fields.get('pkgbase') != self.package or fields.get('pkgver') != expected_version or fields.get('pkgrel') != '1':
            raise ValueError('AUR package metadata differs from recorded candidate identity')
        self.version = expected_version + '-1'

    def result(self, stage, **extra):
        return dict(channel=self.channel, package=self.package, version=self.version,
                    source_commit=self.manifest['source_commit'], stage=stage,
                    candidate_sha256=hashlib.sha256((self.directory / 'candidate.json').read_bytes()).hexdigest(), **extra)

    def preflight(self):
        if not self.local:
            if not self.known_hosts.is_file() or not self.known_hosts.read_text().strip():
                raise ValueError('missing pinned AUR host trust; maintainer must repair known_hosts explicitly')
            try:
                repositories = run(*self.ssh, 'aur@aur.archlinux.org', 'list-repos', env=self.env)
            except ValueError as error:
                raise ValueError('AUR credential/host preflight failed; check AUR_SSH_PRIVATE_KEY and pinned host trust: ' + str(error)) from error
            # aurweb list-repos prefixes maintained repositories with space or *.
            names = {line.strip().lstrip('*').strip() for line in repositories.splitlines()}
            if self.package not in names:
                raise ValueError(f'AUR credential lacks maintainer permission for {self.package}; repair account permissions explicitly')
        # --dry-run exercises actual receive access without changing any ref.
        with tempfile.TemporaryDirectory(prefix='pomotui-aur-preflight-') as tmp:
            run('git', 'clone', '--quiet', self.remote, tmp, env=self.env)
            if self._has_head(tmp):
                run('git', 'push', '--dry-run', self.remote, 'HEAD:master', cwd=tmp, env=self.env)
            elif self.local:
                remote = Path(urllib.parse.urlparse(self.remote).path if self.remote.startswith('file://') else self.remote)
                if not remote.is_dir() or not os.access(remote, os.W_OK):
                    raise ValueError('isolated AUR target is not writable')
        return self.result('preflight-passed', authorized=True)

    def _has_head(self, repo):
        return subprocess.run(['git', '-C', str(repo), 'rev-parse', '--verify', 'HEAD'], capture_output=True).returncode == 0

    def _accepted(self):
        if not self.acceptance:
            raise ValueError('publication requires real upgrade acceptance receipt')
        receipt = json.loads(self.acceptance.read_text())
        digest = hashlib.sha256((self.directory / 'candidate.json').read_bytes()).hexdigest()
        package = self.manifest['packages'][self.channel]
        evidence = receipt.get('packages', {}).get(self.channel, {})
        if receipt.get('passed') is not True or receipt.get('source_commit') != self.manifest['source_commit'] or receipt.get('candidate_sha256') != digest or evidence.get('passed') is not True or evidence.get('sha256') != self.manifest['artifacts'][package]:
            raise ValueError('failed or mismatched real upgrade acceptance; publication blocked')

    def publish(self):
        self._accepted()
        self.preflight()
        with tempfile.TemporaryDirectory(prefix='pomotui-aur-publish-') as tmp:
            run('git', 'clone', '--quiet', self.remote, tmp, env=self.env)
            if self._has_head(tmp):
                old = (Path(tmp) / '.SRCINFO').read_text()
                old_version = re.search(r'^\s*pkgver = (.+)$', old, re.M)
                same = all((Path(tmp) / name).read_bytes() == (self.directory / self.channel / name).read_bytes() for name in FILES)
                if old_version and not same:
                    prior = old_version.group(1)
                    if prior == self.version.rsplit('-', 1)[0]:
                        raise ValueError('AUR same-version content conflict; preserve already published bytes')
                    if self.channel == 'aur-git':
                        count = re.match(r'r(\d+)\.', prior)
                        current = re.match(r'r(\d+)\.', self.manifest['git_version'])
                        if count and int(count.group(1)) >= int(current.group(1)):
                            raise ValueError('AUR newer or conflicting Git snapshot; refusing stale publication')
                    elif tuple(map(int, prior.split('.'))) >= tuple(map(int, self.manifest['version'].split('.'))):
                        raise ValueError('AUR newer stable version; refusing stale publication')
            for name in FILES:
                (Path(tmp) / name).write_bytes((self.directory / self.channel / name).read_bytes())
            run('git', 'add', *FILES, cwd=tmp, env=self.env)
            if run('git', 'diff', '--cached', '--name-only', cwd=tmp, env=self.env):
                run('git', '-c', 'user.name=Pomotui release', '-c', 'user.email=pomotui-release@users.noreply.github.com',
                    'commit', '-qm', f'Update {self.package} to {self.version}\n\nSource: {self.manifest["source_commit"]}', cwd=tmp, env=self.env)
                # Normal fast-forward push, never force. A concurrent update rejects.
                run('git', 'push', self.remote, 'HEAD:master', cwd=tmp, env=self.env)
        return self.verify()

    def verify(self):
        with tempfile.TemporaryDirectory(prefix='pomotui-aur-verify-') as tmp:
            run('git', 'clone', '--quiet', '--branch', 'master', self.remote, tmp, env=self.env)
            commit = run('git', 'rev-parse', 'HEAD', cwd=tmp, env=self.env)
            for name in FILES:
                if (Path(tmp) / name).read_bytes() != (self.directory / self.channel / name).read_bytes():
                    raise ValueError(f'AUR remote content conflict: {self.package}/{name}; no overwrite performed')
        query = urllib.parse.urlencode({'arg[]': self.package})
        url = self.rpc_url + ('&' if '?' in self.rpc_url else '?') + query
        error = None
        for attempt in range(self.discovery_attempts):
            try:
                with urllib.request.urlopen(url, timeout=20) as response:
                    data = json.load(response)
                if any(item.get('Name') == self.package and item.get('Version') == self.version for item in data.get('results', [])):
                    return self.result('discoverable', remote_commit=commit)
            except (OSError, ValueError) as failure:
                error = str(failure)
            if attempt + 1 < self.discovery_attempts:
                time.sleep(self.discovery_delay)
        return self.result('pending-indexing', remote_commit=commit,
                           diagnosis=error or 'authoritative remote verified; AUR index still exposes an older version',
                           retry='verify (same candidate, no push or version change)')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['preflight', 'publish', 'verify'])
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--channel', choices=PACKAGES, required=True)
    parser.add_argument('--remote')
    parser.add_argument('--rpc-url')
    parser.add_argument('--known-hosts', type=Path)
    parser.add_argument('--acceptance', type=Path)
    parser.add_argument('--discovery-attempts', type=int, default=6)
    parser.add_argument('--discovery-delay', type=float, default=10)
    args = parser.parse_args()
    channel = AurChannel(args.candidate, args.channel, args.remote, args.rpc_url, args.known_hosts,
                         args.acceptance, args.discovery_attempts, args.discovery_delay)
    print(json.dumps(getattr(channel, args.action)()))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error))
