"""Candidate CLI tests use disposable source repositories, never release remotes."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
CLI = ROOT / 'packaging/release/candidate.py'


class CandidateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name) / 'source'
        self.repo.mkdir()
        for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']:
            shutil.copy(ROOT / name, self.repo / name)
        shutil.copytree(ROOT / 'packaging', self.repo / 'packaging')
        for crate in (ROOT / 'crates').iterdir():
            dest = self.repo / 'crates' / crate.name
            dest.mkdir(parents=True)
            shutil.copy(crate / 'Cargo.toml', dest / 'Cargo.toml')
        self.git('init', '-q')
        self.git('config', 'user.email', 'candidate@example.invalid')
        self.git('config', 'user.name', 'Candidate test')
        self.git('add', '.')
        self.git('commit', '-qm', 'candidate source')
        self.output = Path(self.tmp.name) / 'candidate'

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.repo), *args], text=True).strip()

    def cli(self, *args, ok=True):
        result = subprocess.run(['python3', str(CLI), *args], text=True, capture_output=True)
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
        return result

    def plan(self, *args, ok=True):
        return self.cli('plan', '--repo', str(self.repo), '--source', 'HEAD',
                        '--output', str(self.output), *args, ok=ok)

    def test_ordinary_source_candidate_has_only_git_channel_and_no_stable_intent(self):
        self.plan()
        manifest = json.loads((self.output / 'candidate.json').read_text())
        self.assertEqual(manifest['required_channels'], ['aur-git'])
        self.assertIsNone(manifest['tag_intent'])
        self.assertEqual(manifest['source_commit'], self.git('rev-parse', 'HEAD'))
        self.assertEqual(manifest['git_version'], 'r1.g' + self.git('rev-parse', '--short=12', 'HEAD'))
        self.assertEqual(manifest['state'], 'planned')

    def test_explicit_stable_intent_derives_version_tag_and_source_metadata(self):
        notes = Path(self.tmp.name) / 'notes.md'
        notes.write_text('Release notes chosen in the release PR.\n')
        self.plan('--intent', 'stable', '--notes', str(notes))
        manifest = json.loads((self.output / 'candidate.json').read_text())
        self.assertEqual(manifest['tag_intent'], 'v0.3.0')
        self.assertEqual(manifest['required_channels'], ['github-linux', 'aur-stable', 'aur-git'])
        recipe = (self.output / 'aur-stable/PKGBUILD').read_text()
        self.assertIn('pkgver=0.3.0\n', recipe)
        self.assertIn('/releases/download/v0.3.0/pomotui-0.3.0-source.tar.gz', recipe)
        self.assertNotIn("sha256sums=('SKIP')", recipe)
        self.assertEqual((self.output / 'aur-stable/pomotui.install').read_bytes(),
                         (self.output / 'aur-git/pomotui.install').read_bytes())
        self.assertIn('pkgver = r1.g', (self.output / 'aur-git/.SRCINFO').read_text())
        self.assertTrue((self.output / 'pomotui-0.3.0-source.tar.gz').is_file())
        self.cli('verify', '--output', str(self.output))

    def test_inconsistent_workspace_lockfile_is_rejected(self):
        lock = self.repo / 'Cargo.lock'
        lock.write_text(lock.read_text().replace('name = "pomotui-cli"\nversion = "0.3.0"',
                                                 'name = "pomotui-cli"\nversion = "0.2.0"'))
        self.git('add', '.')
        self.git('commit', '-qm', 'inconsistent lockfile')
        result = self.plan(ok=False)
        self.assertIn('lockfile version', result.stderr)

    def test_existing_stable_tag_cannot_identify_another_commit(self):
        self.git('tag', 'v0.3.0')
        (self.repo / 'change').write_text('different source')
        self.git('add', '.')
        self.git('commit', '-qm', 'changed source')
        notes = Path(self.tmp.name) / 'notes'
        notes.write_text('Chosen notes')
        result = self.plan('--intent', 'stable', '--notes', str(notes), ok=False)
        self.assertIn('tag v0.3.0', result.stderr)
        self.assertNotEqual(self.git('rev-parse', 'v0.3.0'), self.git('rev-parse', 'HEAD'))

    def test_modified_recipe_or_source_cannot_pass_candidate_verification(self):
        self.plan()
        (self.output / 'aur-git/PKGBUILD').write_text('different recipe')
        result = self.cli('verify', '--output', str(self.output), ok=False)
        self.assertIn('artifact identity mismatch', result.stderr)

    def test_planned_candidate_is_not_build_validated(self):
        self.plan()
        result = self.cli('verify', '--output', str(self.output), '--require-built', ok=False)
        self.assertIn('not built', result.stderr)

    def test_build_label_without_real_outputs_cannot_pass_verification(self):
        self.plan()
        path = self.output / 'candidate.json'
        manifest = json.loads(path.read_text())
        manifest['state'] = 'built'
        path.write_text(json.dumps(manifest))
        result = self.cli('verify', '--output', str(self.output), '--require-built', ok=False)
        self.assertIn('build evidence', result.stderr)

    def test_recorded_version_must_match_the_actual_source_archive(self):
        self.plan()
        path = self.output / 'candidate.json'
        manifest = json.loads(path.read_text())
        manifest['version'] = '9.9.9'
        path.write_text(json.dumps(manifest))
        result = self.cli('verify', '--output', str(self.output), ok=False)
        self.assertIn('source identity mismatch', result.stderr)


if __name__ == '__main__':
    unittest.main()
