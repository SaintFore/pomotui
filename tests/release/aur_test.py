"""Publication boundary tests: isolated Git remotes and a local AUR RPC server."""
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import unittest

sys.path.insert(0, str(Path(__file__).parent))
import candidate_test
from publication_fixture import built_candidate, acceptance

ROOT = Path(__file__).resolve().parents[2]
CLI = ROOT / 'packaging/release/aur.py'


class AurTest(candidate_test.CandidateTest):
    # Inherit source fixture helpers, not the separate candidate test cases.
    def setUp(self):
        super().setUp()
        self.plan()
        manifest = built_candidate(self.output)
        self.acceptance = self.output.parent / 'acceptance.json'
        self.acceptance.write_text(json.dumps(acceptance(self.output)))
        self.remote = self.output.parent / 'aur.git'
        subprocess.run(['git', 'init', '--bare', '--initial-branch=master', '-q', str(self.remote)], check=True)
        self.visible_version = manifest['git_version'] + '-1'
        owner = self
        class Rpc(BaseHTTPRequestHandler):
            def do_GET(self):
                body = json.dumps({'results':[{'Name':'pomotui-git', 'Version':owner.visible_version}]}).encode()
                self.send_response(200)
                self.end_headers()
                self.wfile.write(body)
            def log_message(self, *_):
                pass
        self.http = ThreadingHTTPServer(('127.0.0.1', 0), Rpc)
        threading.Thread(target=self.http.serve_forever, daemon=True).start()
        self.addCleanup(self.http.server_close)
        self.addCleanup(self.http.shutdown)

    def aur(self, action, *args, ok=True, env=None):
        result = subprocess.run([sys.executable, str(CLI), action, '--candidate', str(self.output),
            '--channel', 'aur-git', '--remote', str(self.remote),
            '--rpc-url', f'http://127.0.0.1:{self.http.server_port}/rpc/v5/info',
            '--acceptance', str(self.acceptance), '--discovery-attempts', '1', *args], text=True, capture_output=True, env=env)
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr)
            return json.loads(result.stdout)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        return result

    def test_normal_git_orchestrator_publishes_the_exact_checked_snapshot(self):
        manifest = json.loads((self.output / 'candidate.json').read_text())
        checks = self.output.parent / 'checks.json'
        checks.write_text(json.dumps(dict(passed=True, source_commit=manifest['source_commit'])))
        result = subprocess.run([sys.executable, str(ROOT / 'packaging/release/git_refresh.py'), 'publish',
            '--source', manifest['source_commit'], '--candidate', str(self.output),
            '--acceptance', str(self.acceptance), '--checks', str(checks),
            '--remote', str(self.remote), '--rpc-url', f'http://127.0.0.1:{self.http.server_port}/rpc/v5/info'],
            text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        published = json.loads(result.stdout)
        self.assertEqual(published['stage'], 'discoverable')
        self.assertEqual(published['source_commit'], manifest['source_commit'])

    def test_normal_git_orchestrator_refuses_failed_source_checks(self):
        manifest = json.loads((self.output / 'candidate.json').read_text())
        checks = self.output.parent / 'checks.json'
        checks.write_text(json.dumps(dict(passed=False, source_commit=manifest['source_commit'])))
        result = subprocess.run([sys.executable, str(ROOT / 'packaging/release/git_refresh.py'), 'publish',
            '--source', manifest['source_commit'], '--candidate', str(self.output),
            '--acceptance', str(self.acceptance), '--checks', str(checks),
            '--remote', str(self.remote), '--rpc-url', f'http://127.0.0.1:{self.http.server_port}/rpc/v5/info'],
            text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('source checks', result.stderr)
        head = subprocess.run(['git', '--git-dir', str(self.remote), 'rev-parse', '--verify', 'master'], capture_output=True)
        self.assertNotEqual(head.returncode, 0)

    def test_failed_ssh_authentication_has_actionable_diagnosis_and_no_write(self):
        tools = self.output.parent / 'denied-ssh'
        tools.mkdir()
        ssh = tools / 'ssh'
        ssh.write_text('#!/bin/sh\necho "Permission denied (publickey)" >&2\nexit 255\n')
        ssh.chmod(0o755)
        result = self.aur('publish', '--remote', 'ssh://aur@aur.archlinux.org/pomotui-git.git',
                          '--rpc-url', 'https://aur.archlinux.org/rpc/v5/info', ok=False,
                          env={**os.environ, 'PATH':str(tools) + os.pathsep + os.environ['PATH']})
        self.assertIn('AUR_SSH_PRIVATE_KEY', result.stderr)
        self.assertIn('pinned host trust', result.stderr)

    def test_authenticated_key_without_selected_package_permission_cannot_publish(self):
        tools = self.output.parent / 'ssh-fixture'
        tools.mkdir()
        ssh = tools / 'ssh'
        ssh.write_text('#!/bin/sh\nprintf " unrelated-package\\n"\n')
        ssh.chmod(0o755)
        result = self.aur('publish', '--remote', 'ssh://aur@aur.archlinux.org/pomotui-git.git',
                          '--rpc-url', 'https://aur.archlinux.org/rpc/v5/info', ok=False,
                          env={**os.environ, 'PATH':str(tools) + os.pathsep + os.environ['PATH']})
        self.assertIn('lacks maintainer permission for pomotui-git', result.stderr)

    def test_failed_acceptance_blocks_remote_writes(self):
        self.acceptance.write_text('{"passed": false}')
        result = self.aur('publish', ok=False)
        self.assertIn('acceptance', result.stderr)
        head = subprocess.run(['git', '--git-dir', str(self.remote), 'rev-parse', '--verify', 'master'], capture_output=True)
        self.assertNotEqual(head.returncode, 0)

    def test_delayed_discovery_can_resume_without_another_push(self):
        self.visible_version = 'old-1'
        published = self.aur('publish')
        self.assertEqual(published['stage'], 'pending-indexing')
        self.visible_version = json.loads((self.output / 'candidate.json').read_text())['git_version'] + '-1'
        verified = self.aur('verify')
        self.assertEqual(verified['stage'], 'discoverable')
        self.assertEqual(verified['remote_commit'], published['remote_commit'])

    def test_same_snapshot_with_different_published_bytes_is_a_conflict(self):
        published = self.aur('publish')
        clone = self.output.parent / 'other'
        subprocess.run(['git', 'clone', '-q', str(self.remote), str(clone)], check=True)
        (clone / 'pomotui.install').write_text('different already published hook')
        subprocess.run(['git', '-C', str(clone), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(clone), '-c', 'user.name=Other', '-c', 'user.email=o@example.invalid', 'commit', '-qm', 'conflicting publication'], check=True)
        subprocess.run(['git', '-C', str(clone), 'push', '-q', 'origin', 'HEAD:master'], check=True)
        conflict = subprocess.check_output(['git', '-C', str(clone), 'rev-parse', 'HEAD'], text=True).strip()
        result = self.aur('publish', ok=False)
        self.assertIn('conflict', result.stderr)
        self.assertEqual(subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'master'], text=True).strip(), conflict)

    def test_published_snapshot_is_verified_and_discoverable_with_current_shared_hook(self):
        result = self.aur('publish')
        self.assertEqual(result['stage'], 'discoverable')
        actual = subprocess.check_output(['git', '--git-dir', str(self.remote), 'show', 'master:pomotui.install'])
        self.assertEqual(actual, (self.output / 'aur-git/pomotui.install').read_bytes())
        recipe = subprocess.check_output(['git', '--git-dir', str(self.remote), 'show', 'master:PKGBUILD'], text=True)
        self.assertIn('git+https://github.com/SaintFore/pomotui.git', recipe)
        self.assertNotIn('#commit=', recipe)

# Candidate cases belong to their own suite; retain only channel tests here.
for _name in list(candidate_test.CandidateTest.__dict__):
    if _name.startswith('test_'):
        setattr(AurTest, _name, None)

if __name__ == '__main__':
    unittest.main()
