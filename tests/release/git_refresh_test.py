"""Main selection uses committed changes, never mutable recipes or message text."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
CLI = ROOT / 'packaging/release/git_refresh.py'

class GitRefreshTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        self.git('init', '-q', '--initial-branch=main')
        self.git('config', 'user.name', 'Selection fixture')
        self.git('config', 'user.email', 'selection@example.invalid')
        (self.repo / 'README.md').write_text('Documentation')
        self.git('add', '.')
        self.git('commit', '-qm', 'initial')
        self.before = self.git('rev-parse', 'HEAD')

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.repo), *args], text=True).strip()

    def change(self, path):
        target = self.repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text('changed')
        self.git('add', '.')
        self.git('commit', '-qm', 'update')
        return self.git('rev-parse', 'HEAD')

    def select(self, source):
        result = subprocess.run(['python3', str(CLI), 'select', '--repo', str(self.repo),
            '--source', source, '--before', self.before], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_documentation_and_generated_index_metadata_do_not_refresh_again(self):
        self.change('docs/release.md')
        source = self.change('packaging/aur/pomotui-git/.SRCINFO')
        self.assertFalse(self.select(source)['selected'])

    def test_source_merge_selects_exact_candidate_commit(self):
        source = self.change('crates/pomotui-service/src/main.rs')
        result = self.select(source)
        self.assertTrue(result['selected'])
        self.assertEqual(result['source_commit'], source)
        self.assertEqual(result['intent'], 'git')

if __name__ == '__main__':
    unittest.main()
