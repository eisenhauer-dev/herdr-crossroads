"""Opt-in host binding contracts; never invoke Herdr or use personal config."""

from pathlib import Path, PureWindowsPath
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
COMMAND = ("herdr plugin pane open --plugin herdr-crossroads --entrypoint crossroads"
           + ("-windows" if os.name == "nt" else ""))
AGENT_COMMAND = ("herdr plugin pane open --plugin herdr-crossroads --entrypoint crossroads-agent"
                 + ("-windows" if os.name == "nt" else ""))


class SetupKeybindTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="crossroads-keybind-",
                                                   dir=None if os.name == "nt" else "/tmp/opencode")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.config = self.root / "config.toml"
        self.env = {**{key: os.environ[key] for key in ("SYSTEMROOT",) if key in os.environ},
                    "HOME": str(self.root / "home"), "PATH": "",
                    "HERDR_CONFIG_PATH": str(self.config), "PYTHONDONTWRITEBYTECODE": "1"}

    def invoke(self, env=None, success=True):
        result = subprocess.run([sys.executable, str(SCRIPT)], cwd=self.root,
                                env=self.env if env is None else env,
                                capture_output=True, text=True, encoding="utf-8", timeout=10)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("Keybinding setup failed:", result.stderr)
        return result

    def entry(self, key="prefix+f", kind="shell", command=COMMAND):
        return (f'[[keys.command]]\nkey = "{key}"\ntype = "{kind}"\n'
                f'command = "{command}"\n').encode()

    def write_config(self, data):
        self.config.write_bytes(data.encode() if isinstance(data, str) else data)
        if os.name == "nt":
            from windows_security_fixture import auto_inherit
            auto_inherit(self.config)

    def assert_not_rewritten(self, before):
        after = self.config.stat()
        # Reading legitimately updates atime; writes/replacement must not occur.
        for field in ["st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns"]:
            self.assertEqual(getattr(after, field), getattr(before, field), field)

    def test_manifest_exposes_only_explicit_setup_not_install_or_startup(self):
        manifest = tomllib.loads((ROOT / "herdr-plugin.toml").read_text())
        self.assertEqual(manifest.get("actions", []), [{
            "id": "setup-keybind",
            "title": "Set up prefix+f Select and prefix+a Agent bindings",
            "contexts": ["global"],
            "platforms": ["linux", "macos", "windows"],
            "command": ["./target/release/herdr-crossroads", "--setup-keybind"],
        }])
        self.assertEqual(manifest["build"], [{
            "platforms": ["linux", "macos"],
            "command": ["python3", "scripts/install-binary.py"],
        }, {
            "platforms": ["windows"],
            "command": ["python", "scripts/install-binary.py"],
        }])
        self.assertNotIn("startup", manifest)

    def test_new_config_is_restrictive_and_contains_exact_binding(self):
        self.invoke()
        self.assertEqual(tomllib.loads(self.config.read_text(encoding="utf-8"))["keys"]["command"], [{
            "key": "prefix+f", "type": "shell", "command": COMMAND,
            "description": "Open Herdr Crossroads",
        }, {
            "key": "prefix+a", "type": "shell", "command": AGENT_COMMAND,
            "description": "Open Herdr Crossroads — Agent",
        }])
        if os.name != "nt":
            self.assertEqual(stat.S_IMODE(self.config.stat().st_mode), 0o600)
        self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])

    def test_preserves_original_bytes_comments_permissions_and_is_idempotent(self):
        original = (b'# comment\r\n[general]\r\nname = "test"\r\n'
                    b'[keys]\r\ngoto = "prefix+g"\r\nclose_pane = "prefix+x" # keep')
        self.write_config(original)
        self.config.chmod(0o640)
        before = self.config.stat()
        self.invoke()
        data = self.config.read_bytes()
        self.assertTrue(data.startswith(original))
        if os.name != "nt":
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
                self.write_config(original)
                self.config.chmod(0o640)
                self.invoke()
                data = self.config.read_bytes()
                helper = runpy.run_path(str(SCRIPT))
                self.assertEqual(data, original + b"".join(helper["BINDINGS"].values()))
                self.assertEqual(tomllib.loads(data.decode())["keys"]["indexed"],
                                 tomllib.loads(original.decode())["keys"]["indexed"])
                if os.name != "nt":
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
        if os.name != "nt":
            self.assertEqual(stat.S_IMODE((self.root / "home/.config").stat().st_mode), 0o700)

    def test_reading_old_config_access_time_is_not_a_concurrent_edit(self):
        self.write_config(b"# old file\n")
        os.utime(self.config, (1, 1))
        self.invoke()
        self.assertTrue(self.config.read_bytes().startswith(b"# old file\n"))

    def test_nonascii_path_and_comments_preserve_bytes_and_report_path(self):
        self.config = self.root / "配置 with spaces" / "config.toml"
        self.config.parent.mkdir()
        self.env["HERDR_CONFIG_PATH"] = str(self.config)
        original = '# 配置\r\n[keys]\r\ngoto = "prefix+g" # keep'.encode()
        self.write_config(original)
        self.assertIn(str(self.config), self.invoke().stdout)
        self.assertTrue(self.config.read_bytes().startswith(original))
        before = self.config.read_bytes()
        self.assertIn("Already configured", self.invoke().stdout)
        self.assertEqual(self.config.read_bytes(), before)

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
        if os.name != "nt":
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
        for key, command, other, other_command in [
                ("prefix+f", COMMAND, "prefix+a", AGENT_COMMAND),
                ("prefix+a", AGENT_COMMAND, "prefix+f", COMMAND)]:
            entry = self.entry(key=key, command=command)
            for data in [entry, entry.replace(b'type = "shell"\n', b""),
                         entry.replace(f'"{key}"'.encode(),
                                       f'["prefix+y", " prefix+ {key[-1]} "]'.encode())]:
                with self.subTest(data=data):
                    data += self.entry(key=other, command=other_command)
                    self.write_config(data)
                    before = self.config.stat()
                    self.invoke()
                    self.assertEqual(self.config.read_bytes(), data)
                    self.assert_not_rewritten(before)

    def test_uppercase_modified_and_direct_keys_do_not_collide(self):
        for key in ["prefix+F", "prefix+shift+f", "prefix+ctrl+f", "prefix+control+f",
                    "Prefix+f", "f", "ctrl+f", "prefix+A", "prefix+shift+a",
                    "prefix+ctrl+a", "prefix+control+a", "Prefix+a", "a", "ctrl+a"]:
            with self.subTest(key=key):
                original = self.entry(key=key, command="other")
                self.write_config(original)
                self.invoke()
                self.assertTrue(self.config.read_bytes().startswith(original))
                self.assertEqual(len(tomllib.loads(self.config.read_text(encoding="utf-8"))["keys"]["command"]), 3)
        for prefix in ['"F"', '["ctrl+b", "ctrl+f"]', '"A"', '["ctrl+b", "ctrl+a"]']:
            self.write_config(f"[keys]\nprefix = {prefix}\n")
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

    def test_free_and_correct_mixed_states_append_only_missing_bindings(self):
        bindings = {"prefix+f": COMMAND, "prefix+a": AGENT_COMMAND}
        for existing in [[], ["prefix+f"], ["prefix+a"], list(bindings)]:
            with self.subTest(existing=existing):
                original = b"# retained\r\n" + b"".join(
                    self.entry(key=key, command=bindings[key]) for key in existing)
                self.write_config(original)
                before = self.config.stat()
                self.invoke()
                data = self.config.read_bytes()
                self.assertTrue(data.startswith(original))
                commands = tomllib.loads(data.decode())["keys"]["command"]
                self.assertEqual({entry["key"]: entry["command"] for entry in commands}, bindings)
                self.assertEqual(len(commands), 2)
                if len(existing) == 2:
                    self.assert_not_rewritten(before)
                before = self.config.stat()
                self.assertIn("Already configured", self.invoke().stdout)
                self.assertEqual(self.config.read_bytes(), data)
                self.assert_not_rewritten(before)

    def test_conflicts_on_either_key_prevent_all_appends_even_after_correct_entries(self):
        bindings = {"prefix+f": COMMAND, "prefix+a": AGENT_COMMAND}
        for key, command in bindings.items():
            letter = key[-1]
            other = "prefix+a" if letter == "f" else "prefix+f"
            conflicts = [
                self.entry(key=key, command="other"),
                self.entry(key=key, command=bindings[other]),
                self.entry(key=key, kind="pane", command=command),
                self.entry(key=key, command=command + " --agent"),
                self.entry(key=key, command=command.removesuffix("-windows")
                           if os.name == "nt" else command + "-windows"),
                self.entry(key=f"  prefix+ {letter}  ", command="other"),
                self.entry(key=key, command="other").replace(
                    f'"{key}"'.encode(), f'["prefix+y", " prefix+ {letter} "]'.encode()),
                f'[keys]\nhelp = ["prefix+?", " {key} "]\n'.encode(),
                f'[keys]\nprefix = ["ctrl+b", " {letter} "]\n'.encode(),
                f'[keys]\nextra_prefixes = ["{letter}"]\n'.encode(),
            ]
            for conflict in conflicts:
                for correct in [b"", self.entry(key=other, command=bindings[other]),
                                self.entry(key=key, command=command),
                                b"".join(self.entry(key=k, command=c)
                                         for k, c in bindings.items())]:
                    # Built-ins must precede the array-of-table entries in TOML.
                    for data in [conflict + correct, correct + conflict
                                 if not conflict.startswith(b"[keys]\n") else conflict + correct]:
                        with self.subTest(key=key, data=data):
                            self.write_config(data)
                            before = self.config.stat()
                            result = self.invoke(success=False)
                            self.assertIn(f"{key} is occupied or reserved", result.stderr)
                            self.assertEqual(self.config.read_bytes(), data)
                            self.assert_not_rewritten(before)
                            self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])

    def test_identical_duplicate_and_array_bindings_still_check_other_entries(self):
        for key, command in [("prefix+f", COMMAND), ("prefix+a", AGENT_COMMAND)]:
            other = "prefix+a" if key == "prefix+f" else "prefix+f"
            other_command = AGENT_COMMAND if key == "prefix+f" else COMMAND
            duplicate = self.entry(key=key, command=command) * 2
            array = self.entry(key=key, command=command).replace(
                f'"{key}"'.encode(), f'["{key}", " prefix+ {key[-1]} "]'.encode())
            for entries in [duplicate, array]:
                original = entries + self.entry(key=other, command=other_command)
                self.write_config(original)
                before = self.config.stat()
                self.invoke()
                self.assertEqual(self.config.read_bytes(), original)
                self.assert_not_rewritten(before)
                data = original + self.entry(key=key, command="other")
                self.write_config(data)
                self.invoke(success=False)
                self.assertEqual(self.config.read_bytes(), data)
        # One array cannot launch two different entrypoints correctly.
        self.write_config(self.entry().replace(b'"prefix+f"', b'["prefix+f", "prefix+a"]'))
        self.assertIn("prefix+a is occupied or reserved", self.invoke(success=False).stderr)

    @unittest.skipIf(os.name == "nt", "Unix staging boundary; native Windows guards tested separately")
    def test_conflict_is_detected_before_any_write_open_or_staging(self):
        helper = runpy.run_path(str(SCRIPT))
        real_open = os.open

        def read_only_open(path, flags, *args, **kwargs):
            self.assertFalse(flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC),
                             "conflicts must be checked before a writable file is opened")
            return real_open(path, flags, *args, **kwargs)

        for key, data in [
                ("prefix+f", self.entry(command="other")),
                ("prefix+a", self.entry(key="prefix+a", command="other")),
                ("prefix+f", self.entry(command="other")
                 + self.entry(key="prefix+a", command="other"))]:
            self.write_config(data)
            with mock.patch.dict(os.environ, self.env, clear=True), \
                    mock.patch.object(os, "open", side_effect=read_only_open), \
                    mock.patch.object(helper["secrets"], "token_hex") as staging:
                with self.assertRaisesRegex(ValueError, re.escape(key) + " is occupied"):
                    helper["setup"]()
                staging.assert_not_called()
            self.assertEqual(self.config.read_bytes(), data)

    @unittest.skipIf(os.name == "nt", "Unix link-following policy; Windows rejects reparse points")
    def test_relative_absolute_and_chained_symlinks_preserve_links_and_target_bytes(self):
        helper = runpy.run_path(str(SCRIPT))
        original = b'# dotfiles\r\n[keys]\r\ngoto = "prefix+g" # keep'
        target = self.root / "dotfiles/config.toml"
        target.parent.mkdir()
        intermediate = self.root / "dotfiles/next.toml"
        intermediate.symlink_to("../dotfiles/config.toml")
        for destination in ["dotfiles/config.toml", target, "dotfiles/next.toml"]:
            with self.subTest(destination=destination):
                if self.config.is_symlink():
                    self.config.unlink()
                target.write_bytes(original)
                target.chmod(0o640)
                self.config.symlink_to(destination)
                links = {link: (link.readlink(), link.lstat().st_ino)
                         for link in [self.config, intermediate]}
                result = self.invoke()
                self.assertIn(str(self.config), result.stdout)
                self.assertIn(str(target), result.stdout)
                self.assertEqual(target.read_bytes(), original + b"".join(helper["BINDINGS"].values()))
                self.assertEqual(stat.S_IMODE(target.stat().st_mode), 0o640)
                before = self.config.stat()
                result = self.invoke()
                self.assertIn("Already configured", result.stdout)
                self.assertIn(str(self.config), result.stdout)
                self.assertIn(str(target), result.stdout)
                self.assert_not_rewritten(before)
                self.assertEqual(target.read_bytes(), original + b"".join(helper["BINDINGS"].values()))
                for link, identity in links.items():
                    self.assertEqual((link.readlink(), link.lstat().st_ino), identity)
                self.assertEqual(list(target.parent.glob(".crossroads-keybind-*")), [])
                self.config.unlink()

    @unittest.skipIf(os.name == "nt", "Unix link-following policy; Windows rejects reparse points")
    def test_symlinked_parent_supports_existing_and_new_config(self):
        target = self.root / "dotfiles"
        target.mkdir()
        link = self.root / "link"
        link.symlink_to("dotfiles", target_is_directory=True)
        identity = (link.readlink(), link.lstat().st_ino)
        for suffix in ["config.toml", "new/herdr/config.toml"]:
            with self.subTest(suffix=suffix):
                path = target / suffix
                if suffix == "config.toml":
                    path.write_bytes(b"# keep\n")
                self.invoke({**self.env, "HERDR_CONFIG_PATH": str(link / suffix)})
                self.assertTrue(path.is_file())
                self.assertEqual((link.readlink(), link.lstat().st_ino), identity)

    @unittest.skipIf(os.name == "nt", "Unix FIFO/link policy; native Windows rejection tested separately")
    def test_dangling_loop_and_nonregular_targets_are_refused_without_artifacts(self):
        missing = self.root / "missing/target.toml"
        directory = self.root / "directory"
        directory.mkdir()
        fifo = self.root / "fifo"
        os.mkfifo(fifo)
        loop = self.root / "loop"
        loop.symlink_to("config.toml")
        for destination in [missing, "loop", "config.toml", directory, fifo]:
            with self.subTest(destination=destination):
                self.config.symlink_to(destination)
                identity = (self.config.readlink(), self.config.lstat().st_ino)
                result = self.invoke(success=False)
                self.assertNotIn("Traceback", result.stderr)
                self.assertEqual((self.config.readlink(), self.config.lstat().st_ino), identity)
                self.assertFalse(missing.parent.exists())
                self.assertEqual(list(self.root.rglob(".crossroads-keybind-*")), [])
                self.config.unlink()
        for kind in ["directory", "fifo"]:
            with self.subTest(direct=kind):
                if kind == "directory":
                    self.config.mkdir()
                else:
                    os.mkfifo(self.config)
                self.invoke(success=False)
                if kind == "directory":
                    self.config.rmdir()
                else:
                    self.config.unlink()
        broken_parent = self.root / "broken"
        broken_parent.symlink_to("missing")
        self.invoke({**self.env, "HERDR_CONFIG_PATH": str(broken_parent / "config.toml")},
                    success=False)
        self.assertFalse(missing.parent.exists())

    @unittest.skipIf(os.name == "nt", "Unix link-following policy; Windows pins/rejects ancestors")
    def test_symlink_chain_retargeted_during_staging_is_not_written(self):
        helper = runpy.run_path(str(SCRIPT))
        original = self.root / "original.toml"
        other = self.root / "other.toml"
        intermediate = self.root / "next.toml"
        original.write_bytes(b"# original\n")
        other.write_bytes(b"# other\n")
        self.config.symlink_to("next.toml")
        real_fsync = os.fsync

        def redirect_during_staging(descriptor):
            intermediate.unlink()
            intermediate.symlink_to(destination)
            real_fsync(descriptor)

        for destination in ["other.toml", "missing/target.toml"]:
            with self.subTest(destination=destination):
                if intermediate.is_symlink():
                    intermediate.unlink()
                intermediate.symlink_to("original.toml")
                with mock.patch.dict(os.environ, self.env, clear=True), \
                        mock.patch.object(os, "fsync", side_effect=redirect_during_staging):
                    with self.assertRaisesRegex(ValueError, "config changed before write"):
                        helper["setup"]()
                self.assertEqual(original.read_bytes(), b"# original\n")
                self.assertEqual(other.read_bytes(), b"# other\n")
                self.assertEqual(self.config.readlink(), Path("next.toml"))
                self.assertEqual(intermediate.readlink(), Path(destination))
                self.assertFalse((self.root / "missing").exists())
                self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])
                intermediate.unlink()

    @unittest.skipIf(os.name == "nt", "Unix descriptor policy; native Windows rename denial tested separately")
    def test_resolved_parent_replaced_during_staging_is_not_written(self):
        helper = runpy.run_path(str(SCRIPT))
        parent = self.root / "dotfiles"
        parent.mkdir()
        target = parent / "config.toml"
        target.write_bytes(b"# original\n")
        self.config.symlink_to("dotfiles/config.toml")
        moved = self.root / "moved"
        real_fsync = os.fsync

        def replace_parent_during_staging(descriptor):
            parent.rename(moved)
            parent.mkdir()
            target.write_bytes(b"# replacement\n")
            real_fsync(descriptor)

        with mock.patch.dict(os.environ, self.env, clear=True), \
                mock.patch.object(os, "fsync", side_effect=replace_parent_during_staging):
            with self.assertRaisesRegex(ValueError, "config changed before write"):
                helper["setup"]()
        self.assertEqual((moved / "config.toml").read_bytes(), b"# original\n")
        self.assertEqual(target.read_bytes(), b"# replacement\n")
        self.assertEqual(self.config.readlink(), Path("dotfiles/config.toml"))
        self.assertEqual(list(self.root.rglob(".crossroads-keybind-*")), [])

    def test_atomic_staging_and_replace_failures_preserve_previous_file(self):
        helper = runpy.run_path(str(SCRIPT))
        original = b"# keep me\n"
        for operation in ["fsync", "replace"]:
            with self.subTest(operation=operation):
                self.write_config(original)
                native = (runpy.run_path(str(ROOT / "scripts/windows-files.py"))
                          if os.name == "nt" else None)
                failure = mock.patch.object(
                    native["KERNEL"] if native and operation == "replace" else os,
                    "ReplaceFileW" if native and operation == "replace" else operation,
                    side_effect=OSError("injected failure"))
                with mock.patch.dict(os.environ, self.env, clear=True), \
                        failure, mock.patch.object(runpy, "run_path", return_value=native):
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
                    self.write_config(b"# original\n")
                with mock.patch.dict(os.environ, self.env, clear=True), \
                        mock.patch.object(os, "fsync", side_effect=edit_during_staging):
                    with self.assertRaisesRegex(ValueError, "config changed before write"):
                        helper["setup"]()
                self.assertEqual(self.config.read_bytes(), changed)
                self.assertEqual(list(self.root.glob(".crossroads-keybind-*")), [])

    def test_exclusive_staging_collision_never_removes_someone_elses_file(self):
        helper = runpy.run_path(str(SCRIPT))
        self.write_config(b"# original\n")
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
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        examples = [block for block in re.findall(r"```toml\n(.*?)```", readme, re.S)
                    if "[[keys.command]]" in block]
        helper = runpy.run_path(str(SCRIPT))
        self.assertEqual(len(examples), 2)
        for example, suffix in zip(examples, ("", "-windows")):
            expected = tomllib.loads(b"".join(helper["BINDINGS"].values()).decode())
            for entry in expected["keys"]["command"]:
                entry["command"] = entry["command"].removesuffix("-windows") + suffix
            self.assertEqual(tomllib.loads(example), expected)
        invoke = "herdr plugin action invoke herdr-crossroads.setup-keybind"
        logs = "herdr plugin log list --plugin herdr-crossroads"
        reload = "herdr server reload-config"
        self.assertLess(readme.index(invoke), readme.index(logs))
        self.assertLess(readme.index(logs), readme.index(reload))


class WindowsConfigPathTests(unittest.TestCase):
    def test_windows_commands_use_both_explicit_windows_entrypoints(self):
        with mock.patch.object(os, "name", "nt"):
            helper = runpy.run_path(str(SCRIPT))
        self.assertEqual(helper["COMMANDS"], {
            "f": "herdr plugin pane open --plugin herdr-crossroads --entrypoint crossroads-windows",
            "a": "herdr plugin pane open --plugin herdr-crossroads --entrypoint crossroads-agent-windows",
        })
        data = b"".join(helper["BINDINGS"].values())
        self.assertEqual(helper["missing_bindings"](data), b"")

    def select(self, env):
        helper = runpy.run_path(str(SCRIPT))
        function = helper["config_path"]
        with mock.patch.dict(function.__globals__, WINDOWS=True, Path=PureWindowsPath), \
                mock.patch.dict(os.environ, env, clear=True), \
                mock.patch("tempfile.gettempdir", return_value="C:/wrong-python-temp"), \
                mock.patch.object(runpy, "run_path", return_value={
                    "temp_directory": lambda: PureWindowsPath("C:/isolated-temp")}):
            return function()

    def test_windows_environment_precedence_matches_herdr(self):
        env = {"HERDR_CONFIG_PATH": "D:/配置/config.toml", "XDG_CONFIG_HOME": "D:/xdg",
               "APPDATA": "C:/roaming", "USERPROFILE": "C:/profile", "HOME": "C:/home"}
        for variable, expected in [
                ("HERDR_CONFIG_PATH", "D:/配置/config.toml"),
                ("XDG_CONFIG_HOME", "D:/xdg/herdr/config.toml"),
                ("APPDATA", "C:/roaming/herdr/config.toml"),
                ("USERPROFILE", "C:/profile/AppData/Roaming/herdr/config.toml"),
                ("HOME", "C:/home/.config/herdr/config.toml")]:
            with self.subTest(variable=variable):
                self.assertEqual(self.select(env), PureWindowsPath(expected))
                del env[variable]
        self.assertEqual(self.select(env), PureWindowsPath("C:/isolated-temp/herdr/config.toml"))

    def test_windows_invalid_selected_path_never_falls_back(self):
        for variable in ("HERDR_CONFIG_PATH", "XDG_CONFIG_HOME", "APPDATA", "USERPROFILE", "HOME"):
            for value in ("", "relative", "C:relative", "\\rooted", "C:/a/../b"):
                with self.subTest(variable=variable, value=value):
                    with self.assertRaisesRegex(ValueError, variable):
                        self.select({variable: value})

    def test_windows_absolute_drive_and_unc_selection(self):
        for value in ("C:/配置/config.toml", r"\\server\share\herdr\config.toml"):
            self.assertEqual(self.select({"HERDR_CONFIG_PATH": value}), PureWindowsPath(value))
        for value in ("C:/", "C:/directory/", "C:\\directory\\"):
            with self.assertRaises(ValueError):
                self.select({"HERDR_CONFIG_PATH": value})


if __name__ == "__main__":
    unittest.main()
