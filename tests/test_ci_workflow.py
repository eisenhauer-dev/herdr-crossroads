"""Textual workflow contract checks, not a YAML or Actions schema validator."""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]


class CiWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / ".github/workflows/ci.yml").read_text()

    def test_only_main_branch_pushes_trigger_not_develop_features_or_tags(self):
        push = self.workflow.split("  push:\n", 1)[1].split("  pull_request:\n", 1)[0]
        self.assertEqual(push, "    branches: [main]\n")

    def test_unfiltered_own_and_fork_pull_requests_and_manual_dispatch(self):
        events = self.workflow.split("\npermissions:\n", 1)[0]
        self.assertEqual(events, '''name: CI

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
''')

    def test_single_public_only_read_only_linux_job(self):
        header = self.workflow.split("permissions:\n", 1)[1].split("    steps:\n", 1)[0]
        self.assertEqual(header, '''  contents: read

jobs:
  linux:
    name: Linux gates
    if: github.event.repository.private == false
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    env:
      RUSTUP_TOOLCHAIN: "1.99.0"
''')
        self.assertEqual(re.findall(r"^  [\w-]+:$", self.workflow.split("jobs:\n", 1)[1], re.M), ["  linux:"])
        for forbidden in ["pull_request_target", "secrets.", "continue-on-error:"]:
            self.assertNotIn(forbidden, self.workflow)

    def test_checkout_is_pinned_without_persisted_credentials(self):
        self.assertEqual(re.findall(r"uses: (\S+)", self.workflow), [
            "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
            "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9",
        ])
        self.assertIn("        with:\n          persist-credentials: false\n", self.workflow)

    def test_all_gates_run_in_order_without_partial_keymap_flag(self):
        commands = [
            "python3 tests/test_ci_workflow.py",
            "cargo fmt --check",
            "cargo clippy --locked --all-targets -- -D warnings",
            "cargo test --locked",
            "python3 tests/test_git_hooks.py",
            "python3 tests/test_version.py",
            "python3 tests/test_build_linux.py",
            "python3 tests/test_install_binary.py",
            "python3 tests/test_distribution_helpers.py",
            "sh scripts/build-linux.sh",
        ]
        actual = re.findall(r"^\s+(?:run: )?((?:cargo |python3 tests/|sh scripts/).*)$", self.workflow, re.M)
        self.assertEqual(actual, ["cargo --version", *commands])
        self.assertNotIn("--input-protocol", self.workflow)
        self.assertNotIn("tests/test_distribution.py", self.workflow)
        self.assertNotIn("tests/test_herdr_install.py", self.workflow)
        for preparation in [
            'rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --component rustfmt --component clippy',
            "python3 -c 'import sys; assert sys.version_info >= (3, 11), sys.version'",
            "test ! -L /tmp/opencode",
            "mkdir -p -m 700 /tmp/opencode",
            "test -O /tmp/opencode && test -w /tmp/opencode",
        ]:
            self.assertIn(preparation, self.workflow)
            self.assertLess(self.workflow.index(preparation), self.workflow.index(commands[0]))

    def test_upload_only_fixed_tested_package_and_checksum_after_gates(self):
        upload = self.workflow.split("      - name: Upload tested Linux build\n", 1)
        self.assertEqual(len(upload), 2, "tested artifact upload must exist")
        self.assertEqual(upload[1], '''        uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
        with:
          name: ${{ steps.package.outputs.artifact }}
          path: |
            target/builds/${{ steps.package.outputs.artifact }}.tar.gz
            target/builds/${{ steps.package.outputs.artifact }}.tar.gz.sha256
          if-no-files-found: error
          retention-days: 7
''')
        self.assertIn('''        id: package
        run: |
          version=$(python3 scripts/check-version.py)
          sh scripts/build-linux.sh
          printf 'artifact=herdr-crossroads-%s-x86_64-unknown-linux-gnu\\n' "$version" >> "$GITHUB_OUTPUT"
''', upload[0])
        self.assertNotIn("target/builds/*", self.workflow)
        self.assertNotIn("if: always()", self.workflow)
        self.assertNotIn("target/release/", self.workflow)


if __name__ == "__main__":
    unittest.main()
