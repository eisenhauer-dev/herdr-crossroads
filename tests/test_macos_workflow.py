"""Textual native distribution contract, not YAML/Actions or runtime evidence."""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]


class MacosWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / ".github/workflows/macos.yml").read_text()

    def test_main_pushes_all_pull_requests_and_manual_dispatch(self):
        self.assertEqual(self.workflow.split("\npermissions:\n", 1)[0], '''name: macOS preparation

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
''')

    def test_public_read_only_native_standard_runners_without_fail_fast(self):
        header = self.workflow.split("permissions:\n", 1)[1].split("    steps:\n", 1)[0]
        self.assertEqual(header, '''  contents: read

jobs:
  macos:
    name: macOS preparation (${{ matrix.host }})
    if: github.event.repository.private == false
    runs-on: ${{ matrix.runner }}
    timeout-minutes: 30
    strategy:
      fail-fast: false
      matrix:
        include:
          - runner: macos-15
            machine: arm64
            host: aarch64-apple-darwin
          - runner: macos-15-intel
            machine: x86_64
            host: x86_64-apple-darwin
    defaults:
      run:
        shell: bash
    env:
      RUSTUP_TOOLCHAIN: "1.99.0"
      MACOSX_DEPLOYMENT_TARGET: "15.0"
      EXPECTED_MACHINE: ${{ matrix.machine }}
      EXPECTED_HOST: ${{ matrix.host }}
''')

    def test_pinned_actions_without_credentials_or_publication(self):
        self.assertEqual(re.findall(r"uses: (\S+)", self.workflow), [
            "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
            "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9",
        ])
        self.assertIn("        with:\n          persist-credentials: false\n", self.workflow)
        self.assertEqual(re.findall(r"^  [\w-]+:$", self.workflow.split("jobs:\n", 1)[1], re.M), ["  macos:"])
        for forbidden in [
            "pull_request_target", "secrets.", "continue-on-error:", "if: always()",
            "-large", "-xlarge", "brew ", "scripts/build-linux.sh",
            "cargo install", "--input-protocol", "gh release", "contents: write",
        ]:
            self.assertNotIn(forbidden, self.workflow)

    def test_native_prerequisites_and_safe_leaf_precede_gates(self):
        commands = [
            'test "$(uname -s)" = Darwin',
            'test "$(uname -m)" = "$EXPECTED_MACHINE"',
            "xcrun --find clang",
            "xcrun --show-sdk-path",
            'rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --component rustfmt --component clippy',
            'rustc -vV | grep -Fx "host: $EXPECTED_HOST"',
            "python3 -c 'import sys; assert sys.version_info >= (3, 11), sys.version; import pty, termios, tomllib'",
            "test ! -L /tmp/opencode",
            "mkdir -p -m 700 /tmp/opencode",
            "test -O /tmp/opencode && test -w /tmp/opencode",
        ]
        positions = [self.workflow.index(command) for command in commands]
        self.assertEqual(positions, sorted(positions))
        self.assertLess(positions[-1], self.workflow.index("python3 tests/test_macos_workflow.py"))
        # macOS /tmp itself is an alias; only reject a symlink at the harness leaf.
        self.assertNotIn("test ! -L /tmp\n", self.workflow)

    def test_full_release_gates_in_order(self):
        commands = [
            "python3 tests/test_macos_workflow.py",
            "cargo fmt --check",
            "cargo clippy --locked --all-targets -- -D warnings",
            "cargo test --locked",
            "python3 tests/test_version.py",
            "python3 tests/test_build_macos.py",
            "python3 tests/test_build_provenance.py",
            "python3 tests/test_install_binary.py",
            "python3 tests/test_setup_keybind.py",
            "python3 tests/test_release_install_helpers.py",
        ]
        actual = re.findall(r"^\s+(?:run: )?((?:cargo |python3 (?:-u )?tests/).*)$", self.workflow, re.M)
        self.assertEqual(actual, ["cargo --version", *commands,
            'python3 -u tests/test_release_install.py target/herdr-acceptance "target/builds/${{ steps.package.outputs.artifact }}.tar.gz"'])
        self.assertNotIn("cargo build --release", self.workflow)
        self.assertNotIn("target/release/herdr-crossroads", self.workflow)

    def test_native_packaging_then_real_installation_then_exact_upload(self):
        reader = "python3 -B -m unittest discover -s tests -p test_pty_readers.py"
        self.assertIn(reader, self.workflow)
        self.assertLess(self.workflow.index(reader), self.workflow.index("sh scripts/build-macos.sh"))
        commands = ["sh scripts/build-macos.sh", "Prepare pinned Herdr for isolated installation",
                    "Install and test packaged binary through real Herdr", "Upload tested macOS package and evidence"]
        positions = [self.workflow.index(item) for item in commands]
        self.assertEqual(positions, sorted(positions))
        self.assertIn('printf \'artifact=herdr-crossroads-%s-%s\\n\' "$version" "$EXPECTED_HOST" >> "$GITHUB_OUTPUT"', self.workflow)
        self.assertIn("fb77c2232a05a8035421382c894392f4174e4e38a18e48af3dd0290157c3a7e2", self.workflow)
        self.assertIn("5fb0a908ec7f6745e3e245738b06bfad56c46391c391ab3c0955fbead01f1369", self.workflow)
        self.assertIn('assert hashlib.sha256(data).hexdigest() == expected', self.workflow)
        self.assertIn('with path.open("xb") as output:', self.workflow)
        for suffix in ("tar.gz", "tar.gz.sha256", "provenance.json", "link.map"):
            self.assertIn("target/builds/${{ steps.package.outputs.artifact }}." + suffix, self.workflow)
        self.assertIn("if-no-files-found: error", self.workflow)
        self.assertNotIn("target/builds/*", self.workflow)


if __name__ == "__main__":
    unittest.main()
