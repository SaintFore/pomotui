#!/usr/bin/env python3
"""Prepare source-identified Linux candidates. This program never publishes."""
import argparse
import gzip
import hashlib
import io
import os
import re
import tarfile
import json
from pathlib import Path
import subprocess
import tomllib


def run(*args, cwd=None):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_manifest(output, manifest):
    (output / 'candidate.json').write_text(json.dumps(manifest, indent=2) + '\n')


def plan(args):
    repo = args.repo.resolve()
    commit = run('git', 'rev-parse', f'{args.source}^{{commit}}', cwd=repo)
    if run('git', 'rev-parse', '--is-shallow-repository', cwd=repo) != 'false':
        raise ValueError('full source history is required for a Git snapshot version')
    count = run('git', 'rev-list', '--count', commit, cwd=repo)
    version = tomllib.loads(run('git', 'show', f'{commit}:Cargo.toml', cwd=repo))['workspace']['package']['version']
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('workspace version must be a stable semantic version')
    if args.intent == 'stable':
        tag = f'v{version}'
        existing = subprocess.run(['git', 'rev-parse', '--verify', f'refs/tags/{tag}^{{commit}}'], cwd=repo, text=True, capture_output=True)
        if existing.returncode == 0 and existing.stdout.strip() != commit:
            raise ValueError(f'existing tag {tag} identifies another source commit; never repoint it')
    args.output.mkdir(parents=True)
    manifest = dict(schema=1, state='planned', source_commit=commit, version=version,
                    git_version=f'r{count}.g{commit[:12]}', tag_intent=None,
                    required_channels=['aur-git'], artifacts={}, target='x86_64-unknown-linux-gnu')
    data = subprocess.check_output(['git', 'archive', '--format=tar', f'--prefix=pomotui-{version}/', commit], cwd=repo)
    source_name = f'pomotui-{version}-source.tar.gz'
    (args.output / source_name).write_bytes(gzip.compress(data, mtime=0))
    manifest['source_archive'] = source_name
    manifest['source_archive_sha256'] = sha(args.output / source_name)
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        def source(path):
            return archive.extractfile(f'pomotui-{version}/{path}').read().decode()
        cargo = tomllib.loads(source('Cargo.toml'))
        locked = {p['name']: p['version'] for p in tomllib.loads(source('Cargo.lock'))['package'] if 'source' not in p}
        for member in cargo['workspace']['members']:
            package = tomllib.loads(source(f'{member}/Cargo.toml'))['package']
            if package.get('version') != {'workspace': True}:
                raise ValueError(f'{member} must inherit the workspace version')
            if locked.get(package['name']) != version:
                raise ValueError(f"lockfile version mismatch: {package['name']}")
        manifest['expected_outputs'] = {'aur-git': f'built/aur-git/pomotui-git-{manifest["git_version"]}-1-x86_64.pkg.tar.zst'}
        manifest['toolchain'] = tomllib.loads(source('rust-toolchain.toml'))['toolchain']['channel']
        channels = [('aur-git', 'packaging/aur/pomotui-git', manifest['git_version'])]
        if args.intent == 'stable':
            if not args.notes or not args.notes.read_text().strip():
                raise ValueError('stable intent requires nonempty release notes from the release PR')
            manifest['tag_intent'] = f'v{version}'
            manifest['required_channels'] = ['github-linux', 'aur-stable', 'aur-git']
            (args.output / 'release-notes.md').write_bytes(args.notes.read_bytes())
            manifest['expected_outputs']['aur-stable'] = f'built/aur-stable/pomotui-{version}-1-x86_64.pkg.tar.zst'
            manifest['expected_outputs']['github-linux'] = 'pomotui-linux-x86_64.tar.gz'
            channels.append(('aur-stable', 'packaging/aur', version))
        for channel, template, package_version in channels:
            dest = args.output / channel
            dest.mkdir()
            recipe = source(f'{template}/PKGBUILD')
            info = source(f'{template}/.SRCINFO')
            recipe = re.sub(r'^pkgver=.*$', f'pkgver={package_version}', recipe, flags=re.M)
            info = re.sub(r'^(\s*pkgver = ).*$', rf'\g<1>{package_version}', info, flags=re.M)
            if channel == 'aur-stable':
                url = f'https://github.com/SaintFore/pomotui/releases/download/v{version}/{source_name}'
                src = f'pomotui-{version}.tar.gz::{url}'
                checksum = manifest['source_archive_sha256']
                recipe = re.sub(r'^source=.*$', f'source=("{src}")', recipe, flags=re.M)
                recipe = re.sub(r'^sha256sums=.*$', f"sha256sums=('{checksum}')", recipe, flags=re.M)
                info = re.sub(r'^(\s*source = ).*$', rf'\g<1>{src}', info, flags=re.M)
                info = re.sub(r'^(\s*sha256sums = ).*$', rf'\g<1>{checksum}', info, flags=re.M)
            else:
                recipe = recipe.replace("printf 'r%s.%s'", "printf 'r%s.g%s'").replace('git rev-parse --short HEAD', 'git rev-parse --short=12 HEAD')
            recipe = recipe.replace("makedepends=('cargo'", "makedepends=('rustup'")
            info = info.replace('makedepends = cargo', 'makedepends = rustup')
            toolchain = manifest['toolchain']
            target = manifest['target']
            recipe = recipe.replace('    cargo fetch --locked --target "$(rustc -vV | sed -n \'s/host: //p\')"',
                f'    rustup toolchain install {toolchain} --profile minimal\n    rustup target add --toolchain {toolchain} {target}\n    cargo +{toolchain} fetch --locked --target {target}')
            recipe = recipe.replace('cargo build --frozen --release --workspace', f'cargo +{toolchain} build --frozen --release --workspace --target {target}')
            recipe = recipe.replace('cargo test --frozen', f'cargo +{toolchain} test --frozen')
            recipe = recipe.replace('target/release/', f'target/{target}/release/')
            (dest / 'PKGBUILD').write_text(recipe)
            (dest / '.SRCINFO').write_text(info)
            (dest / 'pomotui.install').write_text(source('packaging/aur/pomotui.install'))
    subprocess.run(['git', 'bundle', 'create', str(args.output.resolve() / 'source.bundle'), '--all'], cwd=repo, check=True, capture_output=True)
    for path in sorted(args.output.rglob('*')):
        if path.is_file():
            manifest['artifacts'][str(path.relative_to(args.output))] = sha(path)
    write_manifest(args.output, manifest)


def verify(args):
    manifest = json.loads((args.output / 'candidate.json').read_text())
    archive_name = f"pomotui-{manifest['version']}-source.tar.gz"
    if manifest['source_archive'] != archive_name:
        raise ValueError('source identity mismatch: archive/version')
    with tarfile.open(args.output / archive_name) as archive:
        if archive.pax_headers.get('comment') != manifest['source_commit']:
            raise ValueError('source identity mismatch: archive/commit')
        cargo = tomllib.loads(archive.extractfile(f"pomotui-{manifest['version']}/Cargo.toml").read().decode())
        if cargo['workspace']['package']['version'] != manifest['version']:
            raise ValueError('source identity mismatch: workspace/version')
    if sha(args.output / archive_name) != manifest['source_archive_sha256']:
        raise ValueError('source identity mismatch: archive/checksum')
    if manifest['tag_intent'] not in (None, f"v{manifest['version']}"):
        raise ValueError('source identity mismatch: stable tag/version')
    for name, checksum in manifest['artifacts'].items():
        path = Path(name)
        if path.is_absolute() or '..' in path.parts:
            raise ValueError(f'invalid artifact path: {name}')
        if sha(args.output / name) != checksum:
            raise ValueError(f'artifact identity mismatch: {name}')
    if args.require_built and manifest['state'] != 'built':
        raise ValueError('candidate is not built; planning is not build validation')
    if manifest['state'] == 'built':
        expected_channels = set(manifest['required_channels']) - {'github-linux'}
        if set(manifest.get('packages', {})) != expected_channels or not manifest.get('build_image') or not manifest.get('binary_sha256'):
            raise ValueError('missing real build evidence')
        for channel, name in manifest['packages'].items():
            if name not in manifest['artifacts'] or not (args.output / name).is_file():
                raise ValueError(f'missing package build evidence: {channel}')
        archive = args.output / 'pomotui-linux-x86_64.tar.gz'
        checksum = args.output / 'pomotui-linux-x86_64.sha256'
        if archive.name not in manifest['artifacts'] or checksum.name not in manifest['artifacts']:
            raise ValueError('missing Linux archive build evidence')
        if checksum.read_text() != f'{sha(archive)}  {archive.name}\n':
            raise ValueError('Linux archive checksum mismatch')
        with tarfile.open(archive) as tar:
            for name in ['pomotui', 'pomotui-service', 'pomotui-tui']:
                data = tar.extractfile(name).read()
                if not data.startswith(b'\x7fELF') or hashlib.sha256(data).hexdigest() != manifest['binary_sha256'].get(name):
                    raise ValueError(f'Linux executable build evidence mismatch: {name}')
    print(f"{manifest['state']}: {manifest['source_commit']}")


def build(args):
    args.output = args.output.resolve()
    verify(argparse.Namespace(output=args.output, require_built=False))
    if json.loads((args.output / 'candidate.json').read_text())['state'] == 'built':
        return  # Identical verified reruns retain the successful artifact bytes.
    script = Path(__file__).with_name('build-arch.sh')
    subprocess.run(['docker', 'pull', 'archlinux:base-devel'], check=True)
    image = run('docker', 'image', 'inspect', 'archlinux:base-devel', '--format', '{{index .RepoDigests 0}}')
    with (args.output / 'build.log').open('w') as log:
        subprocess.run(['docker', 'run', '--rm', '-v', f'{args.output}:/candidate',
                        '-v', f'{script}:/build-arch.sh:ro',
                        '-e', f'CANDIDATE_UID={os.getuid()}', '-e', f'CANDIDATE_GID={os.getgid()}', image,
                        'bash', '/build-arch.sh'], stdout=log, stderr=subprocess.STDOUT, check=True)
    manifest = json.loads((args.output / 'candidate.json').read_text())
    manifest['build_image'] = image
    manifest['build_script_sha256'] = sha(script)
    manifest['packages'] = {}
    binaries = {}
    for channel in ('aur-stable', 'aur-git'):
        if not (args.output / channel).is_dir():
            continue
        name = 'pomotui' if channel == 'aur-stable' else 'pomotui-git'
        version = manifest['version'] if channel == 'aur-stable' else manifest['git_version']
        packages = list((args.output / 'built' / channel).glob('*.pkg.tar.zst'))
        # Debug subpackages are not distribution/acceptance artifacts.
        packages = [p for p in packages if '-debug-' not in p.name]
        if len(packages) != 1:
            raise ValueError(f'{channel}: expected one built package')
        package = packages[0]
        info = run('tar', '--zstd', '-xOf', str(package), '.PKGINFO')
        for field, expected in [('pkgname', name), ('pkgver', version + '-1'), ('arch', 'x86_64')]:
            if f'{field} = {expected}' not in info.splitlines():
                raise ValueError(f'{channel}: package metadata mismatch for {field}')
        manifest['packages'][channel] = str(package.relative_to(args.output))
        manifest['artifacts'][str(package.relative_to(args.output))] = sha(package)
        if not binaries:
            for binary in ['pomotui', 'pomotui-service', 'pomotui-tui']:
                data = subprocess.check_output(['tar', '--zstd', '-xOf', str(package), f'usr/bin/{binary}'])
                binaries[binary] = data
        packaged_hook = subprocess.check_output(['tar', '--zstd', '-xOf', str(package), '.INSTALL'])
        if packaged_hook != (args.output / channel / 'pomotui.install').read_bytes():
            raise ValueError(f'{channel}: packaged install hook differs from candidate')
    archive = args.output / 'pomotui-linux-x86_64.tar.gz'
    with tarfile.open(archive, 'w:gz') as output:
        for name, data in binaries.items():
            entry = tarfile.TarInfo(name)
            entry.size, entry.mode, entry.mtime = len(data), 0o755, 0
            output.addfile(entry, io.BytesIO(data))
    manifest['binary_sha256'] = {name: hashlib.sha256(data).hexdigest() for name, data in binaries.items()}
    checksum = args.output / 'pomotui-linux-x86_64.sha256'
    checksum.write_text(f'{sha(archive)}  {archive.name}\n')
    for path in (archive, checksum):
        manifest['artifacts'][path.name] = sha(path)
    manifest['state'] = 'built'
    # Build validation is never installation/upgrade acceptance or publication approval.
    manifest['acceptance'] = 'required'
    write_manifest(args.output, manifest)
    verify(argparse.Namespace(output=args.output, require_built=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    p = commands.add_parser('plan')
    p.add_argument('--repo', type=Path, default=Path('.'))
    p.add_argument('--source', default='HEAD')
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--intent', choices=['git', 'stable'], default='git')
    p.add_argument('--notes', type=Path)
    b = commands.add_parser('build')
    b.add_argument('--output', type=Path, required=True)
    v = commands.add_parser('verify')
    v.add_argument('--output', type=Path, required=True)
    v.add_argument('--require-built', action='store_true')
    args = parser.parse_args()
    try:
        {'plan': plan, 'build': build, 'verify': verify}[args.command](args)
    except (ValueError, OSError, KeyError, tarfile.TarError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'candidate: {error}\n')


if __name__ == '__main__':
    main()
