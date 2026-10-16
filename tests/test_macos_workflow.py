"""Textual preparation contract, not YAML/Actions validation or macOS support proof."""

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
      EXPECTED_MACHINE: ${{ matrix.machine }}
      EXPECTED_HOST: ${{ matrix.host }}
''')

    def test_only_pinned_checkout_without_credentials_or_distribution(self):
        self.assertEqual(re.findall(r"uses: (\S+)", self.workflow), [
            "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
        ])
        self.assertIn("        with:\n          persist-credentials: false\n", self.workflow)
        self.assertEqual(re.findall(r"^  [\w-]+:$", self.workflow.split("jobs:\n", 1)[1], re.M), ["  macos:"])
        for forbidden in [
            "pull_request_target", "secrets.", "continue-on-error:", "if: always()",
            "-large", "-xlarge", "brew ", "upload-artifact", "scripts/build-linux.sh",
            "scripts/install", "cargo install", "--input-protocol", "--target ",
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
            "cargo build --release --locked",
            "python3 -u tests/pty_latency.py target/release/herdr-crossroads",
            "python3 -u tests/pty_render.py target/release/herdr-crossroads",
            "python3 -u tests/pty_keymaps.py target/release/herdr-crossroads",
            "python3 -u tests/pty_hints.py target/release/herdr-crossroads",
        ]
        actual = re.findall(r"^\s+(?:run: )?((?:cargo |python3 (?:-u )?tests/).*)$", self.workflow, re.M)
        self.assertEqual(actual, ["cargo --version", *commands])
        self.assertEqual(self.workflow.split("      - name: ")[-4:], [
            f"Run full release PTY {harness} gate\n"
            "        timeout-minutes: 5\n"
            f"        run: python3 -u tests/pty_{harness}.py target/release/herdr-crossroads\n"
            for harness in ["latency", "render", "keymaps", "hints"]
        ])


if __name__ == "__main__":
    unittest.main()
