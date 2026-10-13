"""Stable release version agreement, using isolated TOML manifests."""

from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class VersionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-version-")
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name)
        (self.repo / "scripts").mkdir()
        shutil.copy2(ROOT / "scripts/check-version.py", self.repo / "scripts/check-version.py")
        self.files = {
            "Cargo.toml": '[package]\nname = "herdr-crossroads"\nversion = "0.1.0"\n',
            "Cargo.lock": 'version = 4\n[[package]]\nname = "other"\nversion = "9.0.0"\n[[package]]\nname = "herdr-crossroads"\nversion = "0.1.0"\n',
            "herdr-plugin.toml": 'version = "0.1.0"\n',
        }
        self.write_files()

    def write_files(self):
        for name, text in self.files.items():
            (self.repo / name).write_text(text)

    def check(self):
        return subprocess.run([sys.executable, str(self.repo / "scripts/check-version.py")],
                              cwd="/tmp/opencode", text=True, capture_output=True)

    def assert_rejected(self):
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "", "invalid versions must never become path metadata")
        self.assertIn("version", result.stderr.lower())
        self.assertNotIn("Traceback", result.stderr)

    def test_agreement_and_root_resolution(self):
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "0.1.0\n")

    def test_each_manifest_mismatch(self):
        for name, original in self.files.items():
            with self.subTest(file=name):
                (self.repo / name).write_text(original.replace('"0.1.0"', '"0.2.0"'))
                self.assert_rejected()
                self.write_files()

    def test_each_file_missing_malformed_or_without_version(self):
        for name in self.files:
            for contents in [None, "[broken", "", 'version = 1\n', 'package = "invalid"\n']:
                with self.subTest(file=name, contents=contents):
                    path = self.repo / name
                    if contents is None:
                        path.unlink()
                    else:
                        path.write_text(contents)
                    self.assert_rejected()
                    self.write_files()

    def test_unsafe_nonstable_and_noncanonical_versions(self):
        for version in ["../escape", "1/2/3", "$(touch escaped)", "0.1.0;echo bad", "0.1.0\nunsafe",
                        "0.1.0-rc.1", "0.1.0+build", "v0.1.0", "01.1.0", "0.1", "", "١.1.0"]:
            with self.subTest(version=version):
                # repr gives valid TOML literal strings except the deliberately malformed newline.
                for name, original in self.files.items():
                    (self.repo / name).write_text(original.replace('"0.1.0"', repr(version)))
                self.assert_rejected()
                self.assertFalse((self.repo / "escaped").exists())

    def test_missing_or_duplicate_package_entry(self):
        for contents in [self.files["Cargo.lock"].replace('"herdr-crossroads"', '"not-crossroads"'),
                         self.files["Cargo.lock"] + '[[package]]\nname = "herdr-crossroads"\nversion = "0.1.0"\n']:
            with self.subTest(contents=contents):
                (self.repo / "Cargo.lock").write_text(contents)
                self.assert_rejected()

    def test_each_version_must_be_a_string(self):
        for name, original in self.files.items():
            with self.subTest(file=name):
                (self.repo / name).write_text(original.replace('"0.1.0"', '1'))
                self.assert_rejected()
                self.write_files()


if __name__ == "__main__":
    unittest.main()
