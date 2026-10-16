"""Opt-in host binding contracts; never invoke Herdr or use personal config."""

from pathlib import Path
import os
import re
import runpy
import stat
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/setup-keybind.py"
COMMAND = "herdr plugin pane open --plugin herdr-crossroads --entrypoint crossroads"


class SetupKeybindTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="crossroads-keybind-", dir="/tmp/opencode")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.config = self.root / "config.toml"
        self.env = {"HOME": str(self.root / "home"), "PATH": "",
                    "HERDR_CONFIG_PATH": str(self.config), "PYTHONDONTWRITEBYTECODE": "1"}

    def invoke(self, env=None, success=True):
        result = subprocess.run([sys.executable, str(SCRIPT)], cwd=self.root,
                                env=self.env if env is None else env,
                                capture_output=True, text=True, timeout=10)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("Keybinding setup failed:", result.stderr)
        return result

    def entry(self, key="prefix+f", kind="shell", command=COMMAND):
        return (f'[[keys.command]]\nkey = "{key}"\ntype = "{kind}"\n'
                f'command = "{command}"\n').encode()

    def assert_not_rewritten(self, before):
        after = self.config.stat()
        # Reading legitimately updates atime; writes/replacement must not occur.
        for field in ["st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns"]:
            self.assertEqual(getattr(after, field), getattr(before, field), field)

    def test_manifest_exposes_only_explicit_setup_not_install_or_startup(self):
        manifest = tomllib.loads((ROOT / "herdr-plugin.toml").read_text())
        self.assertEqual(manifest.get("actions", []), [{
            "id": "setup-keybind",
            "title": "Set up prefix+f launch binding",
            "contexts": ["global"],
            "command": ["python3", "scripts/setup-keybind.py"],
        }])
        self.assertEqual(manifest["build"], [{
            "command": ["python3", "scripts/install-binary.py"],
        }])
        self.assertNotIn("startup", manifest)

    def test_new_config_is_restrictive_and_contains_exact_binding(self):
        self.invoke()
        self.assertEqual(tomllib.loads(self.config.read_text())["keys"]["command"], [{
            "key": "prefix+f", "type": "shell", "command": COMMAND,
            "description": "Open Herdr Crossroads",
        }])
        self.assertEqual(stat.S_IMODE(self.config.stat().st_mode), 0o600)
        self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])

    def test_preserves_original_bytes_comments_permissions_and_is_idempotent(self):
        original = (b'# comment\r\n[general]\r\nname = "test"\r\n'
                    b'[keys]\r\ngoto = "prefix+g"\r\nclose_pane = "prefix+x" # keep')
        self.config.write_bytes(original)
        self.config.chmod(0o640)
        before = self.config.stat()
        self.invoke()
        data = self.config.read_bytes()
        self.assertTrue(data.startswith(original))
        self.assertEqual(stat.S_IMODE(self.config.stat().st_mode), 0o640)
        self.assertNotEqual(self.config.stat().st_ino, before.st_ino)
        after = self.config.stat()
        result = self.invoke()
        self.assertIn("Already configured", result.stdout)
        self.assertEqual(self.config.read_bytes(), data)
        self.assert_not_rewritten(after)

    def test_indexed_modifier_tables_are_preserved_and_setup_is_idempotent(self):
        for table in [b'[keys.indexed]\r\ntabs = "alt" # keep',
                      b'[keys.indexed]\r\ntabs = "alt"\r\nworkspaces = "ctrl"\r\n'
                      b'agents = "shift+super" # keep',
                      b'[keys.indexed]\ntabs = ""\nworkspaces = "  "\nagents = ""',
                      b'[keys]\nindexed = {}',
                      b'[keys]\nindexed = { tabs = " Control + OPTION + Meta + CMD + Command + Hyper " }']:
            with self.subTest(table=table):
                original = b'# indexed shortcuts\r\n[keys]\r\ngoto = "prefix+g"\r\n'
                if table.startswith(b'[keys]\n'):
                    original = b'# indexed shortcuts\r\n'
                original += table
                self.config.write_bytes(original)
                self.config.chmod(0o640)
                self.invoke()
                data = self.config.read_bytes()
                helper = runpy.run_path(str(SCRIPT))
                self.assertEqual(data, original + helper["BINDING"])
                self.assertEqual(tomllib.loads(data.decode())["keys"]["indexed"],
                                 tomllib.loads(original.decode())["keys"]["indexed"])
                self.assertEqual(stat.S_IMODE(self.config.stat().st_mode), 0o640)
                before = self.config.stat()
                self.assertIn("Already configured", self.invoke().stdout)
                self.assertEqual(self.config.read_bytes(), data)
                self.assert_not_rewritten(before)

    def test_malformed_indexed_tables_are_refused_without_writes(self):
        cases = [b'[keys]\nindexed = "alt"', b'[keys]\nindexed = ["alt"]',
                 b'[keys]\nindexed = 1', b'[[keys.indexed]]\ntabs = "alt"',
                 b'[keys.indexed]\nunknown = "alt"']
        for name in ["tabs", "workspaces", "agents"]:
            for value in ['1', 'true', '["alt"]', '{}', '"prefix+f"', '"alt+"',
                          '"alt++ctrl"', '"bogus"', '"alt+1"']:
                cases.append(f'[keys.indexed]\n{name} = {value}'.encode())
        for data in cases:
            with self.subTest(data=data):
                self.config.write_bytes(data)
                before = self.config.stat()
                result = self.invoke(success=False)
                self.assertIn("keys.indexed", result.stderr)
                self.assertEqual(self.config.read_bytes(), data)
                self.assert_not_rewritten(before)
                self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])

    def test_indexed_table_does_not_bypass_launch_conflicts(self):
        for data in [b'[keys.indexed]\ntabs = "alt"\n' + self.entry(command="other"),
                     b'[keys.indexed]\ntabs = "alt"\n[keys]\nhelp = "prefix+f"\n']:
            with self.subTest(data=data):
                self.config.write_bytes(data)
                result = self.invoke(success=False)
                self.assertIn("prefix+f is occupied or reserved", result.stderr)
                self.assertEqual(self.config.read_bytes(), data)

    def test_path_precedence_and_missing_parents(self):
        env = {**self.env, "XDG_CONFIG_HOME": str(self.root / "xdg")}
        self.invoke(env)
        self.assertTrue(self.config.exists())
        del env["HERDR_CONFIG_PATH"]
        self.invoke(env)
        self.assertTrue((self.root / "xdg/herdr/config.toml").exists())
        del env["XDG_CONFIG_HOME"]
        self.invoke(env)
        self.assertTrue((self.root / "home/.config/herdr/config.toml").exists())
        self.assertEqual(stat.S_IMODE((self.root / "home/.config").stat().st_mode), 0o700)

    def test_reading_old_config_access_time_is_not_a_concurrent_edit(self):
        self.config.write_bytes(b"# old file\n")
        os.utime(self.config, (1, 1))
        self.invoke()
        self.assertTrue(self.config.read_bytes().startswith(b"# old file\n"))

    def test_empty_relative_or_traversing_selected_path_never_falls_back(self):
        for variable in ["HERDR_CONFIG_PATH", "XDG_CONFIG_HOME", "HOME"]:
            for value in ["", "relative", str(self.root / ".." / "elsewhere")]:
                with self.subTest(variable=variable, value=value):
                    env = {"PATH": "", variable: value}
                    if variable != "HOME":
                        env["HOME"] = str(self.root / "fallback")
                    result = self.invoke(env, success=False)
                    self.assertIn(variable, result.stderr)
                    self.assertFalse((self.root / "fallback").exists())
        self.invoke({"PATH": ""}, success=False)
        for value in ["/", str(self.root / "missing") + "/"]:
            self.invoke({**self.env, "HERDR_CONFIG_PATH": value}, success=False)
        self.assertFalse((self.root / "missing").exists())

    def test_custom_builtin_array_and_whitespace_collisions_leave_bytes_unchanged(self):
        cases = [self.entry(command="other"), self.entry(kind="popup"),
                 self.entry(key="  prefix+ f  ", command="other"),
                 b'[keys]\ngoto = "prefix+f"\n',
                 b'[keys]\nhelp = ["prefix+?", " prefix+  f "]\n',
                 b'[[keys.command]]\nkey = ["prefix+y", "prefix+f"]\ncommand = "other"\n',
                 b'[keys]\nprefix = " f "\n',
                 b'[keys]\nprefix = ["ctrl+b", " f "]\n',
                 b'[keys]\nextra_prefixes = ["f"]\n',
                 self.entry() + self.entry(command="other"),
                 b'[keys]\nhelp = "prefix+f"\n' + self.entry()]
        for data in cases:
            with self.subTest(data=data):
                self.config.write_bytes(data)
                result = self.invoke(success=False)
                self.assertIn("prefix+f is occupied or reserved", result.stderr)
                self.assertIn("free key manually", result.stderr)
                self.assertEqual(self.config.read_bytes(), data)

    def test_exact_target_with_implicit_shell_or_array_is_noop(self):
        for data in [self.entry(), self.entry().replace(b'type = "shell"\n', b""),
                     self.entry().replace(b'"prefix+f"', b'["prefix+y", " prefix+ f "]')]:
            with self.subTest(data=data):
                self.config.write_bytes(data)
                before = self.config.stat()
                self.invoke()
                self.assertEqual(self.config.read_bytes(), data)
                self.assert_not_rewritten(before)

    def test_uppercase_modified_and_direct_keys_do_not_collide(self):
        for key in ["prefix+F", "prefix+shift+f", "prefix+ctrl+f", "prefix+control+f",
                    "Prefix+f", "f", "ctrl+f"]:
            with self.subTest(key=key):
                original = self.entry(key=key, command="other")
                self.config.write_bytes(original)
                self.invoke()
                self.assertTrue(self.config.read_bytes().startswith(original))
                self.assertEqual(len(tomllib.loads(self.config.read_text())["keys"]["command"]), 2)
        for prefix in ['"F"', '["ctrl+b", "ctrl+f"]']:
            self.config.write_text(f"[keys]\nprefix = {prefix}\n")
            self.invoke()

    def test_invalid_toml_encoding_or_relevant_shapes_fail_closed(self):
        for data in [b"broken = [", b"# \xff", b"keys = 1", b"keys = []",
                     b"[keys]\ncommand = []", b"keys = {}",
                     b"[keys]\ncommand = {}", b"[keys]\nhelp = 1",
                     b"[keys]\nprefix = [1]", b"[keys.help]\nvalue = 'prefix+f'",
                     b"[[keys.command]]\nkey = 1\ncommand = 'other'",
                     b"[[keys.command]]\nkey = 'prefix+f'",
                     b"[[keys.command]]\nkey = 'prefix+f'\ncommand = 1",
                     self.entry(kind="unknown")]:
            with self.subTest(data=data):
                self.config.write_bytes(data)
                self.invoke(success=False)
                self.assertEqual(self.config.read_bytes(), data)
                self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])

    def test_symlink_nonregular_target_and_symlink_parent_are_refused(self):
        other = self.root / "other"
        other.write_bytes(b"# untouched")
        self.config.symlink_to(other)
        self.invoke(success=False)
        self.assertTrue(self.config.is_symlink())
        self.assertEqual(other.read_bytes(), b"# untouched")
        self.config.unlink()
        self.config.symlink_to(self.root / "missing")
        self.invoke(success=False)
        self.assertFalse((self.root / "missing").exists())
        self.config.unlink()
        self.config.mkdir()
        self.invoke(success=False)
        self.config.rmdir()
        os.mkfifo(self.config)
        self.invoke(success=False)
        self.config.unlink()
        link = self.root / "link"
        link.symlink_to(self.root, target_is_directory=True)
        self.invoke({**self.env, "HERDR_CONFIG_PATH": str(link / "config.toml")}, success=False)
        self.assertFalse(self.config.exists())

    def test_atomic_staging_and_replace_failures_preserve_previous_file(self):
        helper = runpy.run_path(str(SCRIPT))
        original = b"# keep me\n"
        for operation in ["fsync", "replace"]:
            with self.subTest(operation=operation):
                self.config.write_bytes(original)
                with mock.patch.dict(os.environ, self.env, clear=True), \
                        mock.patch.object(os, operation, side_effect=OSError("injected failure")):
                    with self.assertRaisesRegex(OSError, "injected failure"):
                        helper["setup"]()
                self.assertEqual(self.config.read_bytes(), original)
                self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])

    def test_concurrent_change_or_creation_is_not_overwritten(self):
        helper = runpy.run_path(str(SCRIPT))
        real_fsync = os.fsync
        changed = b"# concurrent edit\n"

        def edit_during_staging(descriptor):
            self.config.write_bytes(changed)
            real_fsync(descriptor)

        for existing in [False, True]:
            with self.subTest(existing=existing):
                if existing:
                    self.config.write_bytes(b"# original\n")
                with mock.patch.dict(os.environ, self.env, clear=True), \
                        mock.patch.object(os, "fsync", side_effect=edit_during_staging):
                    with self.assertRaisesRegex(ValueError, "config changed before write"):
                        helper["setup"]()
                self.assertEqual(self.config.read_bytes(), changed)
                self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])

    def test_exclusive_staging_collision_never_removes_someone_elses_file(self):
        helper = runpy.run_path(str(SCRIPT))
        self.config.write_bytes(b"# original\n")
        staging = self.root / ".crossroads-keybind-fixed"
        staging.write_bytes(b"# another writer\n")
        with mock.patch.dict(os.environ, self.env, clear=True), \
                mock.patch.object(helper["secrets"], "token_hex", return_value="fixed"):
            with self.assertRaises(FileExistsError):
                helper["setup"]()
        self.assertTrue(staging.exists(), "exclusive-open failure must not remove another writer's file")
        self.assertEqual(staging.read_bytes(), b"# another writer\n")
        self.assertEqual(self.config.read_bytes(), b"# original\n")

    def test_readme_toml_example_matches_setup_and_documents_async_order(self):
        readme = (ROOT / "README.md").read_text()
        example = next(block for block in re.findall(r"```toml\n(.*?)```", readme, re.S)
                       if "[[keys.command]]" in block)
        helper = runpy.run_path(str(SCRIPT))
        self.assertEqual(tomllib.loads(example), tomllib.loads(helper["BINDING"].decode()))
        invoke = "herdr plugin action invoke herdr-crossroads.setup-keybind"
        logs = "herdr plugin log list --plugin herdr-crossroads"
        reload = "herdr server reload-config"
        self.assertLess(readme.index(invoke), readme.index(logs))
        self.assertLess(readme.index(logs), readme.index(reload))


if __name__ == "__main__":
    unittest.main()
