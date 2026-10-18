#!/usr/bin/env python3
"""Publication policy exercised through the public coordinator CLI."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
CLI = ROOT / 'packaging/release/release.py'

class ReleasePolicy(unittest.TestCase):
    def test_an_ordinary_merge_cannot_select_a_stable_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            event = Path(tmp) / 'event.json'
            event.write_text(json.dumps({'action': 'closed', 'pull_request': {'merged': True, 'title': 'Fix startup', 'base': {'ref': 'main'}}}))
            result = subprocess.run(['python3', str(CLI), 'select', '--event', str(event), '--repo', str(ROOT), '--output', str(Path(tmp) / 'selection.json')], text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads((Path(tmp) / 'selection.json').read_text())['intent'], 'git')

if __name__ == '__main__':
    unittest.main()
