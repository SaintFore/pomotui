"""Tiny prebuilt transport fixtures for publication tests, not build/upgrade proof.

Plan a candidate from a disposable committed source first. The production gate's
real makepkg/systemd evidence is deliberately tested separately on hosted Arch.
"""
import hashlib
import io
import json
from pathlib import Path
import tarfile


def built_candidate(output):
    output = Path(output)
    manifest = json.loads((output / 'candidate.json').read_text())
    manifest.update(state='built', build_image='fixture@sha256:test', binary_sha256={})
    archive = output / 'pomotui-linux-x86_64.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        for name in ['pomotui', 'pomotui-service', 'pomotui-tui']:
            data = b'\x7fELFfixture:' + name.encode()
            manifest['binary_sha256'][name] = hashlib.sha256(data).hexdigest()
            entry = tarfile.TarInfo(name)
            entry.size = len(data)
            tar.addfile(entry, io.BytesIO(data))
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum_file = output / 'pomotui-linux-x86_64.sha256'
    checksum_file.write_text(f'{checksum}  {archive.name}\n')
    manifest['packages'] = {}
    artifacts = [archive, checksum_file]
    for channel in set(manifest['required_channels']) - {'github-linux'}:
        name = manifest['expected_outputs'][channel]
        package = output / name
        package.parent.mkdir(parents=True, exist_ok=True)
        package.write_bytes(b'prebuilt package fixture:' + channel.encode())
        manifest['packages'][channel] = name
        artifacts.append(package)
    for path in artifacts:
        manifest['artifacts'][str(path.relative_to(output))] = hashlib.sha256(path.read_bytes()).hexdigest()
    (output / 'candidate.json').write_text(json.dumps(manifest))
    return manifest


def acceptance(output):
    output = Path(output)
    manifest = json.loads((output / 'candidate.json').read_text())
    return dict(passed=True, source_commit=manifest['source_commit'],
        candidate_sha256=hashlib.sha256((output / 'candidate.json').read_bytes()).hexdigest(),
        packages={channel:dict(passed=True, sha256=manifest['artifacts'][name])
                  for channel, name in manifest['packages'].items()})
