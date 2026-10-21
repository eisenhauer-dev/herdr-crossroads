"""Textual Windows preparation contract checks, not a YAML/Actions schema validator."""
from pathlib import Path
import re
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]


class WindowsWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / ".github/workflows/windows.yml").read_text()

    def test_standard_public_only_native_runner_and_read_only_events(self):
        self.assertEqual(self.workflow.split("\npermissions:\n")[0], """name: Windows preparation

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
""")
        for expected in ["permissions:\n  contents: read\n", "if: github.event.repository.private == false",
                         "runs-on: windows-2025", "timeout-minutes: 30", "shell: pwsh",
                         'RUSTUP_TOOLCHAIN: "1.99.0"']:
            self.assertIn(expected, self.workflow)
        for forbidden in ["pull_request_target", "secrets.", "continue-on-error", "if: always()", "windows-latest"]:
            self.assertNotIn(forbidden, self.workflow)

    def test_pinned_actions_and_explicit_preparation_only_artifacts(self):
        self.assertEqual(re.findall(r"uses: (\S+)", self.workflow), [
            "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
            "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9"])
        for expected in ["persist-credentials: false", "name: windows-preparation-", "if-no-files-found: error",
                         "retention-days: 7", ".tar.gz\n", ".tar.gz.sha256\n", ".provenance.json\n", ".link.map\n"]:
            self.assertIn(expected, self.workflow)
        self.assertNotIn("target/builds/*", self.workflow)
        self.assertNotIn("target/release/", self.workflow)
        gates = ["cargo fetch --locked", "python -B tests/test_windows_workflow.py",
                 "python -B tests/test_windows_process.py", "cargo fmt --check",
                 "cargo clippy --locked --lib --bin herdr-crossroads -- -D warnings",
                 "python -B scripts/build-windows.py", "Select exact preparation evidence", "Upload Windows preparation only"]
        self.assertEqual(sorted(gates, key=self.workflow.index), gates)

    def test_manifest_preserves_unix_and_selects_explicit_windows_executable(self):
        manifest = tomllib.loads((ROOT / "herdr-plugin.toml").read_text())
        self.assertEqual(manifest["platforms"], ["linux", "macos", "windows"])
        panes = {pane["id"]: pane for pane in manifest["panes"]}
        self.assertEqual(panes["crossroads"]["platforms"], ["linux", "macos"])
        self.assertEqual(panes["crossroads"]["command"], ["./target/release/herdr-crossroads"])
        self.assertEqual(panes["crossroads-windows"]["platforms"], ["windows"])
        self.assertEqual(panes["crossroads-windows"]["command"], ["./target/release/herdr-crossroads.exe"])
        self.assertEqual(manifest["build"][1], {"platforms": ["windows"], "command": ["python", "scripts/install-binary.py"]})
        self.assertEqual(manifest["actions"][0]["platforms"], ["linux", "macos"])


if __name__ == "__main__":
    unittest.main()
