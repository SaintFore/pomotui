"""Version preflight observes the tests in an isolated source workspace."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class WorkspaceBoundariesTest(unittest.TestCase):
    def test_version_coupled_test_fails_before_build_without_changing_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            subprocess.run(['git', 'init', '-q', str(repo)], check=True)
            manifest = repo / 'Cargo.toml'
            manifest.write_text('[workspace]\nmembers=[]\n[workspace.package]\nversion="0.3.0"\n')
            (repo / 'Cargo.lock').write_text('version = 4\n')
            tests = repo / 'tests/release'
            tests.mkdir(parents=True)
            (tests / 'coupled_test.py').write_text(
                'import pathlib,tomllib,unittest\n'
                'class Coupled(unittest.TestCase):\n'
                ' def test_version(self):\n'
                '  data=tomllib.loads(pathlib.Path("Cargo.toml").read_text())\n'
                '  self.assertEqual(data["workspace"]["package"]["version"],"0.3.0")\n')
            original = manifest.read_bytes()
            result = subprocess.run([sys.executable, str(ROOT / 'tests/release/check-boundaries.py'),
                                     '--repo', str(repo), '--version', '0.0.0'],
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("'0.0.0' != '0.3.0'", result.stdout + result.stderr)
            self.assertEqual(manifest.read_bytes(), original)

    def test_member_lock_versions_change_without_touching_dependencies_or_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            subprocess.run(['git', 'init', '-q', str(repo)], check=True)
            manifest = repo / 'Cargo.toml'
            manifest.write_text('[workspace]\nmembers=["app"]\n[workspace.package]\nversion="0.3.0"\n')
            (repo / 'app').mkdir()
            (repo / 'app/Cargo.toml').write_text('[package]\nname="example"\nversion.workspace=true\n')
            lock = repo / 'Cargo.lock'
            lock.write_text('version = 4\n[[package]]\nname = "example"\nversion = "0.3.0"\n'
                            '[[package]]\nname = "dependency"\nversion = "0.3.0"\nsource = "registry+example"\n')
            tests = repo / 'tests/release'
            tests.mkdir(parents=True)
            (tests / 'versions_test.py').write_text(
                'import pathlib,tomllib,unittest\n'
                'class Versions(unittest.TestCase):\n'
                ' def test_versions(self):\n'
                '  data=tomllib.loads(pathlib.Path("Cargo.toml").read_text())\n'
                '  self.assertEqual(data["workspace"]["package"]["version"],"0.0.0")\n'
                '  packages=tomllib.loads(pathlib.Path("Cargo.lock").read_text())["package"]\n'
                '  self.assertEqual([(p["name"],p["version"]) for p in packages],[("example","0.0.0"),("dependency","0.3.0")])\n')
            originals = {path: path.read_bytes() for path in (manifest, lock)}
            result = subprocess.run([sys.executable, str(ROOT / 'tests/release/check-boundaries.py'),
                                     '--repo', str(repo), '--version', '0.0.0'],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            for path, contents in originals.items():
                self.assertEqual(path.read_bytes(), contents)
