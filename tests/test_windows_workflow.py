"""Textual Windows preparation contract checks, not a YAML/Actions schema validator."""
from pathlib import Path
import ast
import re
import tomllib
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]


class WindowsWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / ".github/workflows/windows.yml").read_text(encoding="utf-8")

    def test_standard_public_only_native_runner_and_read_only_events(self):
        self.assertEqual(self.workflow.split("\npermissions:\n")[0], """name: Windows

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
""")
        self.assertIn("jobs:\n  windows:\n    name: Windows x64 · Build & Tests (x86_64)\n", self.workflow)
        for expected in ["permissions:\n  contents: read\n", "if: github.event.repository.private == false",
                         "runs-on: windows-2025", "timeout-minutes: 30", "shell: pwsh",
                         'RUSTUP_TOOLCHAIN: "1.99.0-x86_64-pc-windows-gnu"',
                         'CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER: "C:/mingw64/bin/gcc.exe"',
                         'RUSTFLAGS: "-C link-self-contained=no"']:
            self.assertIn(expected, self.workflow)
        for forbidden in ["pull_request_target", "secrets.", "continue-on-error", "if: always()", "windows-latest"]:
            self.assertNotIn(forbidden, self.workflow)

    def test_pinned_actions_and_explicit_preparation_only_artifacts(self):
        self.assertEqual(re.findall(r"uses: (\S+)", self.workflow), [
            "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
            "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9"])
        for expected in ["persist-credentials: false", "name: windows-preparation-", "if-no-files-found: error",
                         "retention-days: 7", ".tar.gz\n", ".tar.gz.sha256\n", ".provenance.json\n", ".link.map\n",
                         "Upload Windows preparation only (not release approval)"]:
            self.assertIn(expected, self.workflow)
        self.assertNotIn("target/builds/*", self.workflow)
        self.assertNotIn("target/release/", self.workflow)
        gates = ["cargo fetch --locked", "python -B tests/test_windows_workflow.py",
                  "python -B tests/test_windows_process.py", "python -B tests/test_windows_provenance.py",
                  "python -B tests/test_windows_support.py",
                  "Verify existing GNU tooling",
                  "cargo fmt --check",
                 "cargo clippy --locked --lib --bin herdr-crossroads -- -D warnings",
                 "python -B scripts/build-windows.py", "Select exact preparation evidence", "Upload Windows preparation only"]
        self.assertEqual(sorted(gates, key=self.workflow.index), gates)

    def test_manifest_preserves_unix_and_selects_explicit_windows_executable(self):
        manifest = tomllib.loads((ROOT / "herdr-plugin.toml").read_text(encoding="utf-8"))
        self.assertEqual(manifest["platforms"], ["linux", "macos", "windows"])
        panes = {pane["id"]: pane for pane in manifest["panes"]}
        self.assertEqual(set(panes), {"crossroads", "crossroads-windows", "crossroads-agent", "crossroads-agent-windows"})
        for pane in panes.values():
            self.assertEqual(pane["placement"], "popup")
            self.assertEqual((pane["width"], pane["height"]), ("90%", "90%"))
        self.assertEqual(panes["crossroads"]["platforms"], ["linux", "macos"])
        self.assertEqual(panes["crossroads"]["command"], ["./target/release/herdr-crossroads"])
        self.assertEqual(panes["crossroads-windows"]["platforms"], ["windows"])
        self.assertEqual(panes["crossroads-windows"]["command"], ["./target/release/herdr-crossroads.exe"])
        for entry, platforms, binary in [
                ("crossroads-agent", ["linux", "macos"], "herdr-crossroads"),
                ("crossroads-agent-windows", ["windows"], "herdr-crossroads.exe")]:
            self.assertEqual(panes[entry]["platforms"], platforms)
            self.assertEqual(panes[entry]["command"], ["./target/release/" + binary, "--agent"])
            self.assertEqual(panes[entry]["title"], "Herdr Crossroads — Agent")
        self.assertEqual(manifest["build"][1], {"platforms": ["windows"], "command": ["python", "scripts/install-binary.py"]})
        self.assertEqual(manifest["actions"][0]["platforms"], ["linux", "macos", "windows"])
        self.assertEqual([action["id"] for action in manifest["actions"]], ["setup-keybind"])
        self.assertEqual(manifest["actions"][0]["title"],
                         "Set up prefix+f Select and prefix+a Agent bindings")
        self.assertEqual(manifest["actions"][0]["command"],
                         ["./target/release/herdr-crossroads", "--setup-keybind"])

    def test_native_helper_runs_mouse_and_input_policy_groups_and_both_acceptance_passes(self):
        tree = ast.parse((ROOT / "scripts/build-windows.py").read_text(encoding="utf-8"))
        loop = next(node for node in ast.walk(tree) if isinstance(node, ast.For)
                    and isinstance(node.target, ast.Name) and node.target.id == "group")
        groups = ast.literal_eval(loop.iter)
        for group in ("test_cli", "test_input", "test_search", "test_selection::navigation", "test_mouse",
                      "test_actions::help_enter_restores_origin", "test_keymap::mouse_opt_out",
                      "test_keymap::enter_is_reserved", "test_keymap::lowercase_f",
                      "test_selection::agent", "test_presentation", "test_topology"):
            self.assertIn(group, groups)
        command = loop.body[0].value
        self.assertEqual([value.value if isinstance(value, ast.Constant) else value.id
                          for value in command.args[0].elts],
                         ["cargo", "test", "--locked", "--lib", "group"])
        self.assertEqual(next(k.value.value for k in command.keywords if k.arg == "timeout"), 60)
        acceptance = [node for node in ast.walk(tree) if isinstance(node, ast.Call)
                      and isinstance(node.func, ast.Name) and node.func.id == "run"
                      and node.args and isinstance(node.args[0], ast.List)
                      and any(isinstance(value, ast.Constant) and value.value == "tests/test_windows_acceptance.py"
                              for value in node.args[0].elts)]
        self.assertEqual(len(acceptance), 2, "final-linked and installed bytes both need native mouse acceptance")
        for command in acceptance:
            self.assertEqual(next(k.value.value for k in command.keywords if k.arg == "timeout"), 240)
        source = (ROOT / "scripts/build-windows.py").read_text(encoding="utf-8")
        for expected in ('"--test", "setup_dispatch"', '"tests/test_setup_keybind.py"',
                         '"tests/test_windows_keybind.py"'):
            self.assertIn(expected, source)
        calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call)
                 and isinstance(node.func, ast.Name) and node.func.id == "run"
                 and node.args and isinstance(node.args[0], ast.List)]
        def script_call(path):
            return next(node for node in calls if any(
                isinstance(value, ast.Constant) and value.value == path for value in node.args[0].elts))
        diagnostic = script_call("tests/diagnose_windows_security.py")
        self.assertEqual([value.value if isinstance(value, ast.Constant) else ast.unparse(value)
                          for value in diagnostic.args[0].elts],
                         ["sys.executable", "-u", "-B", "tests/diagnose_windows_security.py"])
        self.assertEqual(next(k.value.value for k in diagnostic.keywords if k.arg == "timeout"), 60)
        self.assertEqual(next(k.value.id for k in diagnostic.keywords if k.arg == "env"), "environment")
        ordered = sorted(calls, key=lambda node: node.lineno)
        self.assertIs(ordered[ordered.index(diagnostic) + 1], script_call("tests/test_setup_keybind.py"))
        self.assertLess(script_call("tests/test_setup_keybind.py").lineno,
                        script_call("tests/test_windows_keybind.py").lineno)


class WindowsWorkflowEncodingTests(unittest.TestCase):
    def test_contracts_preserve_utf8_under_non_utf8_defaults(self):
        original_read_text = Path.read_text
        for default_encoding in ("utf-8", "cp1252"):
            with self.subTest(default_encoding=default_encoding):
                def read_text(path, encoding=None, errors=None):
                    return original_read_text(path, encoding=encoding or default_encoding, errors=errors)

                suite = unittest.defaultTestLoader.loadTestsFromTestCase(WindowsWorkflowTests)
                result = unittest.TestResult()
                with patch.object(Path, "read_text", read_text):
                    suite.run(result)
                self.assertEqual(result.testsRun, 4)
                self.assertEqual(result.errors, [])
                self.assertEqual(result.failures, [])


if __name__ == "__main__":
    unittest.main()
