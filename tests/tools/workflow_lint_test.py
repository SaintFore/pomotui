"""Run the repository's public workflow validator on disposable workflows."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class WorkflowLintTest(unittest.TestCase):
    def lint(self, concurrency='queue: max', step='run: echo checked', cancel='false'):
        with tempfile.TemporaryDirectory() as tmp:
            workflow = Path(tmp) / 'workflow.yml'
            workflow.write_text(
                'name: Example\non: push\njobs:\n  check:\n'
                '    runs-on: ubuntu-latest\n    concurrency:\n'
                f'      group: example\n      {concurrency}\n'
                f'      cancel-in-progress: {cancel}\n    steps:\n'
                f'      - {step}\n')
            return subprocess.run([sys.executable, str(ROOT / 'tools/lint-workflows.py'),
                                   str(workflow)], capture_output=True, text=True)

    def test_documented_max_queue_is_valid(self):
        result = self.lint()
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)


    def test_single_queue_is_valid(self):
        self.assertEqual(self.lint('queue: single').returncode, 0)

    def test_unknown_queue_is_rejected(self):
        result = self.lint('queue: typo')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('queue must be single or max', result.stdout + result.stderr)

    def test_max_queue_cannot_cancel_in_progress(self):
        result = self.lint(cancel='true')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('queue max requires cancel-in-progress false', result.stdout + result.stderr)

    def test_unrelated_expression_checks_remain_active(self):
        result = self.lint(step='run: echo ${{ unknown_context.value }}')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('undefined', result.stdout + result.stderr)
