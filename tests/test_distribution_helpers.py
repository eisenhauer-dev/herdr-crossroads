"""Pure native-harness helper checks; does not launch Herdr or any subprocess."""

import io
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from unittest.mock import Mock, patch

import test_distribution
import test_herdr_install

from test_distribution import (ARCHIVE, Screen, digest, enable_public_downloads,
                               fixture_next_version, prepare_fixture, public_version)

ROOT = Path(__file__).resolve().parents[1]


class DistributionHelpers(unittest.TestCase):
    def test_cli_prepares_offline_metadata_without_changing_public_metadata(self):
        class FixtureChecked(Exception):
            pass

        with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="xrd-cli-helper-") as temporary:
            assets = Path(temporary)
            name = ARCHIVE.format("0.0.1")
            with tarfile.open(assets / name, "w:gz") as package:
                entry = tarfile.TarInfo("herdr-crossroads")
                entry.size = 64
                package.addfile(entry, io.BytesIO(b"\x7fELF" + bytes(60)))
            (assets / (name + ".sha256")).write_text(f"{digest((assets / name).read_bytes())}  {name}\n")

            for public in [False, True]:
                def check_fixture(argv, **kwargs):
                    self.assertEqual(argv, ["/not-executed/herdr", "--version"])
                    repo = kwargs["cwd"] / "source"
                    for filename in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
                        path = repo / filename
                        if public:
                            self.assertEqual(path.read_bytes(), (ROOT / filename).read_bytes())
                        else:
                            metadata = tomllib.loads(path.read_text())
                            version = (metadata["version"] if filename == "herdr-plugin.toml" else
                                       metadata["package"]["version"] if filename == "Cargo.toml" else
                                       next(p["version"] for p in metadata["package"]
                                            if p["name"] == "herdr-crossroads"))
                            self.assertEqual(version, "0.0.1", filename)
                    raise FixtureChecked()

                with self.subTest(public=public), patch("subprocess.Popen", side_effect=check_fixture):
                    with self.assertRaises(FixtureChecked):
                        test_herdr_install.run_case(Path("/not-executed/herdr"), assets, name, "success",
                                                    "v0.0.1" if public else None,
                                                    "a" * 40 if public else None)

    def test_update_checkpoint_accepts_reviewed_post_merge_main_bound_to_release(self):
        release, main = "b" * 40, "d" * 40
        commands = []

        def command(argv):
            commands.append(argv)
            output = (f"ref: refs/heads/main\tHEAD\n{main}\tHEAD\n{main}\trefs/heads/main\n"
                      if "--symref" in argv else f"{release}\t{argv[-2]}\n") if argv[0] == "git" else ""
            return subprocess.CompletedProcess(argv, 0, output, "")

        for predeclared in [None, main]:
            with self.subTest(predeclared=predeclared), patch(
                    "sys.stdin", io.StringIO(f"update {release} {main}\n")), patch("sys.stdout", io.StringIO()):
                result, resolved = test_distribution.public_cycle_step(command, ["herdr"], "update", "v0.0.2", release,
                                                                       predeclared)
                self.assertTrue(any(c[0] == "herdr" for c in commands))
                self.assertEqual(result.returncode, 0)
                self.assertEqual(resolved, main)

    def test_separate_main_identity_does_not_replace_release_tag_binding(self):
        release, main = "a" * 40, "c" * 40
        for operation in ["install", "update", "return"]:
            with self.subTest(operation=operation), patch("sys.stdin", io.StringIO(f"update {main}\n")), patch(
                    "sys.stdout", io.StringIO()):
                command = Mock(side_effect=lambda argv: subprocess.CompletedProcess(
                    argv, 0, f"{main}\t{argv[-2]}\n", ""))
                with self.assertRaisesRegex(ValueError, "approved commit"):
                    test_distribution.public_cycle_step(command, ["herdr"], operation, "v0.0.1", release, main)
                self.assertTrue(all(c.args[0][0] == "git" for c in command.call_args_list))

    def test_cycle_accepts_distinct_reviewed_main_and_release_commits(self):
        release, main = "a" * 40, "c" * 40
        commands = []

        def command(argv):
            commands.append(argv)
            output = (f"ref: refs/heads/main\tHEAD\n{main}\tHEAD\n{main}\trefs/heads/main\n"
                      if "--symref" in argv else f"{release}\t{argv[-2]}\n") if argv[0] == "git" else ""
            return subprocess.CompletedProcess(argv, 0, output, "")

        for operation in ["install", "update", "return"]:
            commands.clear()
            with self.subTest(operation=operation), patch("sys.stdin", io.StringIO(f"update {main}\n")), patch(
                    "sys.stdout", io.StringIO()) as output:
                result, resolved = test_distribution.public_cycle_step(command, ["herdr"], operation,
                                                                       "v0.0.1", release, main)
                self.assertEqual(result.returncode, 0)
                self.assertEqual(resolved, release if operation == "return" else main)
                if operation == "update":
                    self.assertIn(f"update {main}", output.getvalue())
                    self.assertIn("do NOT eliminate branch races", output.getvalue())
            self.assertEqual(sum("--symref" in c for c in commands), 0 if operation == "return" else 2)
            operation_argv = next(c for c in commands if c[0] == "herdr")
            self.assertEqual(operation_argv[-2:] if operation == "return" else operation_argv[-1:],
                             ["--ref", release] if operation == "return" else ["--yes"])

    def test_main_commit_parameters_fail_closed_before_effects(self):
        release_pairs = ["v0.0.1", "a" * 40, "v0.0.2", "b" * 40]
        cases = [(release_pairs, None, "d" * 40),
                 ([None] * 4, "c" * 40, "d" * 40),
                 (release_pairs[:2] + [None, None], "c" * 40, "d" * 40)]
        for invalid in ["", "a" * 7, "A" * 40, "g" * 40, "a" * 41, "a" * 40 + "\n"]:
            cases.extend([(release_pairs, invalid, "d" * 40), (release_pairs, "c" * 40, invalid)])
        for releases, initial, update in cases:
            params = [*releases, initial, update]
            with self.subTest(params=params), patch("tempfile.TemporaryDirectory") as temporary:
                with self.assertRaises(ValueError):
                    test_distribution.run_case(Path("/not-executed/herdr"), Path("/unused"), *params)
                temporary.assert_not_called()
            argv = ["test_distribution.py", "--herdr-bin", "/not-executed/herdr",
                    "--asset-dir", "/not-executed/assets"]
            for flag, value in zip(["--public-ref", "--public-commit", "--public-update-ref",
                                    "--public-update-commit", "--public-main-commit",
                                    "--public-update-main-commit"], params):
                if value is not None:
                    argv.extend([flag, value])
            with self.subTest(argv=argv), patch("sys.argv", argv), patch("sys.stderr", io.StringIO()), patch(
                    "pathlib.Path.resolve") as resolve, patch("test_distribution.run_case") as run:
                with self.assertRaises(SystemExit) as error:
                    test_distribution.main()
                self.assertEqual(error.exception.code, 2)
                resolve.assert_not_called()
                run.assert_not_called()

    def test_invalid_main_commit_rejected_before_cycle_commands(self):
        for operation in ["install", "update", "return"]:
            with self.subTest(operation=operation):
                command = Mock()
                with self.assertRaises(ValueError):
                    test_distribution.public_cycle_step(command, ["herdr"], operation, "v0.0.1", "a" * 40, "C" * 40)
                command.assert_not_called()

    def test_public_cycle_cli_selects_two_reviewed_versions(self):
        argv = ["test_distribution.py", "--herdr-bin", "/not-executed/herdr",
                "--asset-dir", "/not-executed/assets", "--public-ref", "v0.0.1",
                "--public-commit", "a" * 40, "--public-update-ref", "v0.0.2",
                "--public-update-commit", "b" * 40]
        executable = Mock()
        executable.__enter__ = Mock(return_value=io.BytesIO(b"\x7fELF"))
        executable.__exit__ = Mock(return_value=False)
        for extra, expected in [([], (None, None)),
                                (["--public-main-commit", "c" * 40], ("c" * 40, None)),
                                (["--public-main-commit", "c" * 40, "--public-update-main-commit", "d" * 40],
                                 ("c" * 40, "d" * 40))]:
            executable.__enter__.return_value = io.BytesIO(b"\x7fELF")
            with self.subTest(extra=extra), patch("sys.argv", argv + extra), patch("sys.stderr", new_callable=io.StringIO), patch(
                "pathlib.Path.resolve", autospec=True, side_effect=lambda path, **kw: path), patch(
                "pathlib.Path.open", return_value=executable), patch("os.access", return_value=True), patch(
                "pathlib.Path.is_dir", return_value=True), patch("test_distribution.run_case") as run:
                status = 0
                try:
                    test_distribution.main()
                except SystemExit as error:
                    status = error.code
                self.assertEqual(status, 0, "complete reviewed cycle parameters must be accepted")
                run.assert_called_once_with(Path("/not-executed/herdr"), Path("/not-executed/assets"),
                                            "v0.0.1", "a" * 40, "v0.0.2", "b" * 40, *expected)

    def test_public_cli_verifies_tag_before_installing_approved_commit(self):
        approved, moved, tag_object = "a" * 40, "b" * 40, "c" * 40
        tag = "refs/tags/v0.1.0"
        cases = [
            (f"{approved}\t{tag}\n", 0, True),
            (f"{tag_object}\t{tag}\n{approved}\t{tag}^{{}}\n", 0, True),
            (f"{moved}\t{tag}\n", 0, False),
            (f"{approved}\t{tag}\n{moved}\t{tag}^{{}}\n", 0, False),
            ("", 2, False),
            ("", 128, False),
            ("", 0, False),
            (f"{approved}\t{tag}\n{approved}\t{tag}\n", 0, False),
            (f"{approved}\t{tag}^{{}}\n", 0, False),
            (f"{approved}\t{tag}\n{approved}\trefs/tags/v0.1.1\n", 0, False),
            (f"not-a-sha\t{tag}\n", 0, False),
        ]

        class InstallAttempted(Exception):
            pass

        with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="xrd-tag-check-") as temporary:
            assets = Path(temporary)
            name = ARCHIVE.format("0.1.0")
            with tarfile.open(assets / name, "w:gz") as package:
                entry = tarfile.TarInfo("herdr-crossroads")
                entry.size = 64
                package.addfile(entry, io.BytesIO(b"\x7fELF" + bytes(60)))
            (assets / (name + ".sha256")).write_text(f"{digest((assets / name).read_bytes())}  {name}\n")
            for output, status, allowed in cases:
                events = []

                def fake_process(argv, **kwargs):
                    process = Mock(returncode=0)
                    if argv[1:] == ["--version"]:
                        process.communicate.return_value = ("herdr 0.9.5\n", "")
                    elif len(argv) > 1 and argv[1] == "ls-remote":
                        events.append(("verify", argv))
                        process.returncode = status
                        process.communicate.return_value = (output, "")
                    elif argv[1:4] == ["plugin", "install", "herdr-crossroads"]:
                        process.returncode = 2
                        process.communicate.return_value = ("", "usage: install OWNER/REPO")
                    elif argv[1:4] == ["plugin", "install", test_distribution.SOURCE]:
                        events.append(("install", argv))
                        raise InstallAttempted()
                    else:
                        raise AssertionError(f"unexpected command: {argv}")
                    return process

                with self.subTest(output=output, status=status), patch(
                        "subprocess.Popen", side_effect=fake_process), patch(
                        "test_herdr_install.threading.Thread") as thread:
                    thread.return_value.is_alive.return_value = False
                    try:
                        test_herdr_install.run_case(Path("/not-executed/herdr"), assets, name,
                                                    "success", "v0.1.0", approved)
                    except (ValueError, InstallAttempted) as error:
                        caught = error
                    else:
                        self.fail("fixture must stop before any real hook or registration")
                    if allowed:
                        self.assertIsInstance(caught, InstallAttempted)
                        self.assertEqual([kind for kind, _ in events], ["verify", "install"])
                        self.assertEqual(events[-1][1][-2:], ["--ref", approved])
                    else:
                        self.assertIsInstance(caught, ValueError, "unreviewed source reached install hooks")
                        self.assertEqual([kind for kind, _ in events], ["verify"])

    def test_cycle_parameters_fail_closed_before_creating_fixture(self):
        for params in [(None, None, "v0.0.2", "b" * 40),
                       ("v0.0.1", "a" * 40, "v0.0.2", None),
                       ("v0.0.1", "a" * 40, None, "b" * 40),
                       ("v0.0.1", None, None, None),
                       (None, "a" * 40, None, None),
                       ("v0.0.1", "a" * 40, "main", "b" * 40),
                       ("v0.0.1", "a" * 40, "v0.0.2", "a" * 40),
                       ("v0.0.1", "a" * 40, "v0.0.1", "b" * 40),
                       ("v0.1.0", "a" * 40, "v0.0.2", "b" * 40)]:
            with self.subTest(params=params), patch("tempfile.TemporaryDirectory") as temporary:
                with self.assertRaises(ValueError):
                    test_distribution.run_case(Path("/not-executed/herdr"), Path("/unused"), *params)
                temporary.assert_not_called()

    def test_cycle_runs_unpinned_install_update_and_exact_sha_return(self):
        commands = []
        expected = "a" * 40

        def command(argv):
            commands.append(argv)
            if argv[1] == "ls-remote":
                if "--symref" in argv:
                    output = f"ref: refs/heads/main\tHEAD\n{expected}\tHEAD\n{expected}\trefs/heads/main\n"
                else:
                    output = f"{expected}\t{argv[-2]}\n"
            else:
                output = ""
            return subprocess.CompletedProcess(argv, 0, output, "")

        argv = ["/not-executed/herdr", "--session", "owned"]
        test_distribution.public_cycle_step(command, argv, "install", "v0.0.1", expected)
        expected = "b" * 40
        with patch("sys.stdin", io.StringIO(f"update {expected}\n")), patch("sys.stdout", io.StringIO()) as output:
            test_distribution.public_cycle_step(command, argv, "update", "v0.0.2", expected)
            self.assertIn(expected, output.getvalue())
        expected = "a" * 40
        test_distribution.public_cycle_step(command, argv, "return", "v0.0.1", expected)
        operations = [c for c in commands if c[0] != "git"]
        self.assertEqual(operations, [
            [*argv, "plugin", "install", test_distribution.SOURCE, "--yes"],
            [*argv, "plugin", "update", "herdr-crossroads", "--yes"],
            [*argv, "plugin", "install", test_distribution.SOURCE, "--yes", "--ref", expected]])
        self.assertEqual(sum("--symref" in c for c in commands), 4)

    def test_update_checkpoint_rejects_eof_or_unconfirmed_input_without_commands(self):
        for text in ["", "yes\n", "update " + "a" * 40 + "\n", "update " + "b" * 40,
                     f"update {'a' * 40} {'d' * 40}\n", f"update {'b' * 40} {'D' * 40}\n",
                     f"update {'b' * 40} {'d' * 7}\n", f"update {'b' * 40} {'d' * 40}",
                     f"update {'b' * 40} {'d' * 40} extra\n", f"update  {'b' * 40} {'d' * 40}\n"]:
            with self.subTest(text=text), patch("sys.stdin", io.StringIO(text)), patch("sys.stdout", io.StringIO()):
                command = Mock()
                with self.assertRaises(ValueError):
                    test_distribution.public_cycle_step(command, ["herdr"], "update", "v0.0.2", "b" * 40)
                command.assert_not_called()

    def test_update_checkpoint_cannot_override_predeclared_reviewed_main(self):
        release, main = "b" * 40, "c" * 40
        for text in [f"update {release} {'d' * 40}\n", f"update {release}\n"]:
            with self.subTest(text=text), patch("sys.stdin", io.StringIO(text)), patch("sys.stdout", io.StringIO()):
                command = Mock()
                with self.assertRaises(ValueError):
                    test_distribution.public_cycle_step(command, ["herdr"], "update", "v0.0.2", release, main)
                command.assert_not_called()

    def test_unpinned_operations_require_exact_public_main_and_tag(self):
        sha = "a" * 40
        good = f"ref: refs/heads/main\tHEAD\n{sha}\tHEAD\n{sha}\trefs/heads/main\n"
        bad = ["", good.replace("refs/heads/main", "refs/heads/develop"),
               good.replace(sha, "b" * 40), good + f"{sha}\tHEAD\n",
               good + f"{sha}\trefs/heads/other\n", f"{sha}\tHEAD\n",
               good.replace(sha, "invalid", 1)]
        for operation in ["install", "update"]:
            for output, status in [(text, 0) for text in bad] + [(good, 128)]:
                with self.subTest(operation=operation, output=output, status=status):
                    calls = []

                    def command(argv):
                        calls.append(argv)
                        text = output if "--symref" in argv else f"{sha}\t{argv[-2]}\n"
                        return subprocess.CompletedProcess(argv, status if "--symref" in argv else 0, text, "")

                    with patch("sys.stdin", io.StringIO(f"update {sha}\n")), patch("sys.stdout", io.StringIO()):
                        with self.assertRaises(ValueError):
                            test_distribution.public_cycle_step(command, ["herdr"], operation, "v0.0.2", sha)
                    self.assertTrue(all(c[0] == "git" for c in calls))
            command = Mock(return_value=subprocess.CompletedProcess([], 0, "", ""))
            with patch("sys.stdin", io.StringIO(f"update {sha}\n")), patch("sys.stdout", io.StringIO()):
                with self.assertRaises(ValueError):
                    test_distribution.public_cycle_step(command, ["herdr"], operation, "v0.0.2", sha)
            self.assertTrue(all(c.args[0][0] == "git" for c in command.call_args_list))

    def test_unpinned_postcheck_detects_main_movement(self):
        sha = "a" * 40
        events = []

        def command(argv):
            events.append(argv)
            expected = "b" * 40 if any(c[0] == "herdr" for c in events) else sha
            output = (f"ref: refs/heads/main\tHEAD\n{expected}\tHEAD\n{expected}\trefs/heads/main\n"
                      if "--symref" in argv else f"{sha}\t{argv[-2]}\n")
            return subprocess.CompletedProcess(argv, 0, output, "")

        with self.assertRaisesRegex(ValueError, "main"):
            test_distribution.public_cycle_step(command, ["herdr"], "install", "v0.0.1", sha)
        self.assertEqual(sum(c[0] == "herdr" for c in events), 1)

    def test_cycle_verifies_both_release_bindings_before_server_or_install(self):
        events = []

        def fixture(base, *args):
            repo, assets = base / "source", base / "assets"
            repo.mkdir()
            assets.mkdir()
            return repo, assets, {"0.0.1": b"first", "0.0.2": b"second"}

        def process(argv, **kwargs):
            events.append(argv)
            child = Mock(returncode=0)
            if argv[1:] == ["--version"]:
                child.communicate.return_value = ("herdr 0.9.5\n", "")
            elif argv[1] == "ls-remote":
                tag = argv[-2]
                # First approved, second absent: neither install nor server may start.
                child.communicate.return_value = ((f"{'a' * 40}\t{tag}\n" if tag.endswith("v0.0.1") else ""), "")
            else:
                raise AssertionError(f"unexpected executable: {argv}")
            return child

        with patch("test_distribution.prepare_fixture", side_effect=fixture), patch(
                "subprocess.Popen", side_effect=process), patch("sys.stderr", io.StringIO()):
            with self.assertRaisesRegex(ValueError, "approved commit"):
                test_distribution.run_case(Path("/bin/sh"), Path("/unused"),
                                           "v0.0.1", "a" * 40, "v0.0.2", "b" * 40)
        self.assertEqual([c[-2] for c in events if c[1] == "ls-remote"],
                         ["refs/tags/v0.0.1", "refs/tags/v0.0.2"])

    def test_shared_public_install_uses_exact_tag_and_immutable_sha(self):
        approved, tag_object = "a" * 40, "c" * 40
        tag = "refs/tags/v0.1.0"
        commands = []

        def command(argv):
            commands.append(argv)
            output = f"{tag_object}\t{tag}\n{approved}\t{tag}^{{}}\n" if argv[1] == "ls-remote" else ""
            return subprocess.CompletedProcess(argv, 0, output, "")

        result = test_distribution.install_public(command, ["/not-executed/herdr", "--session", "owned"],
                                                   "v0.1.0", approved)
        self.assertEqual(commands, [
            ["git", "ls-remote", "--exit-code", "https://github.com/eisenhauer-dev/herdr-crossroads.git",
             tag, tag + "^{}"],
            ["/not-executed/herdr", "--session", "owned", "plugin", "install",
             test_distribution.SOURCE, "--yes", "--ref", approved]])
        self.assertEqual(result.returncode, 0)

    def test_public_mode_requires_pinned_ref_and_commit(self):
        commit = "a" * 40
        self.assertEqual(public_version("v0.1.0", commit), "0.1.0")
        for ref, sha in [(None, None), ("main", commit), ("v01.2.3", commit),
                         ("v0.1.0-rc.1", commit), ("v0.1.0", None),
                         ("v0.1.0", "a" * 7), ("v0.1.0", "A" * 40)]:
            with self.subTest(ref=ref, sha=sha), self.assertRaises(ValueError):
                public_version(ref, sha)

    def test_public_mode_removes_local_override_and_allows_only_https_git(self):
        with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="xrd-public-helper-") as temporary:
            config = Path(temporary) / "gitconfig"
            config.write_text('[url "file:///fixture"]\n    insteadOf = https://github.com/\n')
            env = {"GIT_CONFIG_GLOBAL": str(config), "GIT_ALLOW_PROTOCOL": "file",
                   "CROSSROADS_ASSET_DIR": "/fixture", "HOME": temporary, "PATH": "/restricted"}
            with patch("subprocess.run", side_effect=AssertionError("must not launch a command")), patch(
                    "subprocess.Popen", side_effect=AssertionError("must not launch Herdr")):
                enable_public_downloads(env)
            self.assertNotIn("CROSSROADS_ASSET_DIR", env)
            self.assertEqual(env["GIT_ALLOW_PROTOCOL"], "https")
            self.assertEqual(env["PATH"], "/restricted")
            self.assertNotIn("insteadOf", config.read_text())
            self.assertIn('allow = never', config.read_text())
            self.assertIn('[protocol "https"]\n    allow = always', config.read_text())
            self.assertIn('hooksPath = /dev/null', config.read_text())
            self.assertIn('[credential]\n    helper =', config.read_text())

    def test_current_cells_replace_old_query_and_rows(self):
        screen = Screen(rows=20, cols=80)
        screen.feed(b"\x1b[2J\x1b[2;5HSearch: / to search       Select\x1b[4;5HOrigin\x1b[5;5HArrival")
        self.assertIn("Origin", screen.popup_view())
        # Feed an incomplete CSI: no historical row can survive the erase.
        screen.feed(b"\x1b[2")
        screen.feed(b"J\x1b[2;5HSearch: Arrival           Search\x1b[4;5HArrival")
        self.assertNotIn("Origin", screen.popup_view())
        self.assertEqual(screen.popup_view().count("Arrival"), 2)
        screen.feed(b"\x1b[2;30HSelect\x1b[0K")
        self.assertRegex(screen.popup_view(), r"Search: Arrival\s+Select")
        # Underlying host labels are not popup rows.
        screen.feed(b"\x1b[20;1HOrigin")
        self.assertIn("Origin", screen.text())
        self.assertNotIn("Origin", screen.popup_view())

    def test_utf8_chunks_line_erase_and_unsupported_controls(self):
        screen = Screen(rows=2, cols=20)
        screen.feed(b"old text\r\x1b[2K")
        marker = "─".encode()
        screen.feed(marker[:1])
        screen.feed(marker[1:] + b"new")
        self.assertTrue(screen.text().startswith("─new"))
        self.assertNotIn("old", screen.text())
        painted = screen.text()
        screen.feed(b"\x1b[>7u\x1b[?u\x1b[<u\x1b[c\x1b[?996n\x1b[16t")
        self.assertEqual(screen.text(), painted)
        with self.assertRaisesRegex(AssertionError, "unsupported native CSI"):
            screen.feed(b"\x1b[1P")

    def test_pixel_mouse_mode_query_does_not_paint_cells(self):
        screen = Screen(rows=2, cols=20)
        screen.feed(b"Current")
        painted, cursor = screen.text(), (screen.y, screen.x)
        screen.feed(b"\x1b[?1016$")
        screen.feed(b"p")
        self.assertEqual(screen.text(), painted)
        self.assertEqual((screen.y, screen.x), cursor)
        with self.assertRaises(AssertionError):
            screen.feed(b"\x1b[?9999$p")

    def test_artifacts_and_versions_stay_in_fixture(self):
        with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="xrd-helper-") as temporary:
            root = Path(temporary)
            trusted = root / "trusted"
            trusted.mkdir()
            payload = b"\x7fELF" + bytes(60)
            name = ARCHIVE.format("0.0.1")
            with tarfile.open(trusted / name, "w:gz") as package:
                for filename, data in [("herdr-crossroads", payload), ("herdr-crossroads.sha256",
                                       f"{digest(payload)}  herdr-crossroads\n".encode()),
                                       ("LICENSE", (ROOT / "LICENSE").read_bytes()),
                                       ("THIRD-PARTY-NOTICES.html", (ROOT / "THIRD-PARTY-NOTICES.html").read_bytes())]:
                    entry = tarfile.TarInfo(filename)
                    entry.size = len(data)
                    package.addfile(entry, io.BytesIO(data))
            checksum = f"{digest((trusted / name).read_bytes())}  {name}\n"
            (trusted / (name + ".sha256")).write_text(checksum)
            base = root / "fixture"
            base.mkdir()
            repo, assets, payloads = prepare_fixture(base, trusted)
            self.assertEqual((assets / name).read_bytes(), (trusted / name).read_bytes())
            self.assertEqual(payloads["0.0.1"], payload)
            self.assertNotEqual(digest(payloads["0.0.1"]), digest(payloads["0.1.1"]))
            for version in ["0.0.1", "0.1.1"]:
                with tarfile.open(assets / ARCHIVE.format(version)) as package:
                    for filename in ["LICENSE", "THIRD-PARTY-NOTICES.html"]:
                        self.assertEqual(package.extractfile(filename).read(), (ROOT / filename).read_bytes())
            # The offline next-version fixture starts at 0.0.1 regardless of checkout version.
            self.assertEqual(tomllib.loads((repo / "Cargo.toml").read_text())["package"]["version"], "0.0.1")
            self.assertEqual(tomllib.loads((repo / "herdr-plugin.toml").read_text())["version"], "0.0.1")
            package = next(p for p in tomllib.loads((repo / "Cargo.lock").read_text())["package"]
                           if p["name"] == "herdr-crossroads")
            self.assertEqual(package["version"], "0.0.1")
            fixture_next_version(repo)
            manifest = tomllib.loads((repo / "herdr-plugin.toml").read_text())
            self.assertEqual(manifest["version"], "0.1.1")
            self.assertEqual(manifest["panes"][0]["width"], "90%")
            self.assertEqual(tomllib.loads((repo / "Cargo.toml").read_text())["package"]["version"], "0.1.1")
            package = next(p for p in tomllib.loads((repo / "Cargo.lock").read_text())["package"]
                           if p["name"] == "herdr-crossroads")
            self.assertEqual(package["version"], "0.1.1")
            self.assertEqual((trusted / (name + ".sha256")).read_text(), checksum)
            public_base = root / "public-reference"
            public_base.mkdir()
            public_repo, public_assets, public_payloads = prepare_fixture(public_base, trusted, "0.0.1")
            self.assertEqual(public_payloads, {"0.0.1": payload})
            self.assertEqual(sorted(p.name for p in public_assets.iterdir()), [name, name + ".sha256"])
            for filename in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
                self.assertEqual((public_repo / filename).read_bytes(), (ROOT / filename).read_bytes())
            self.assertFalse((public_repo / ".git").exists())
            update_name = ARCHIVE.format("0.0.2")
            # Synthetic helper inputs only; never public runtime evidence.
            with tarfile.open(trusted / update_name, "w:gz") as package:
                for filename, data in [("herdr-crossroads", payload + b"second"),
                                       ("herdr-crossroads.sha256", f"{digest(payload + b'second')}  herdr-crossroads\n".encode()),
                                       ("LICENSE", (ROOT / "LICENSE").read_bytes()),
                                       ("THIRD-PARTY-NOTICES.html", (ROOT / "THIRD-PARTY-NOTICES.html").read_bytes())]:
                    entry = tarfile.TarInfo(filename)
                    entry.size = len(data)
                    package.addfile(entry, io.BytesIO(data))
            (trusted / (update_name + ".sha256")).write_text(
                f"{digest((trusted / update_name).read_bytes())}  {update_name}\n")
            before = {p.name: p.read_bytes() for p in trusted.iterdir()}
            cycle = root / "cycle"
            cycle.mkdir()
            _, cycle_assets, cycle_payloads = prepare_fixture(cycle, trusted, "0.0.1", "0.0.2")
            self.assertEqual(cycle_payloads, {"0.0.1": payload, "0.0.2": payload + b"second"})
            self.assertEqual({p.name: p.read_bytes() for p in cycle_assets.iterdir()}, before)
            # Same binary under a different archive name must not pass as an update.
            archive_bytes = (trusted / name).read_bytes()
            (trusted / update_name).write_bytes(archive_bytes)
            (trusted / (update_name + ".sha256")).write_text(f"{digest(archive_bytes)}  {update_name}\n")
            rejected_cycle = root / "same-runtime"
            rejected_cycle.mkdir()
            with self.assertRaisesRegex(AssertionError, "distinct hashes"):
                prepare_fixture(rejected_cycle, trusted, "0.0.1", "0.0.2")
            (trusted / (update_name + ".sha256")).unlink()
            missing_cycle = root / "missing-update-checksum"
            missing_cycle.mkdir()
            with self.assertRaises(FileNotFoundError):
                prepare_fixture(missing_cycle, trusted, "0.0.1", "0.0.2")
            (trusted / (name + ".sha256")).write_text("bad checksum\n")
            other = root / "rejected"
            other.mkdir()
            with self.assertRaises(AssertionError):
                prepare_fixture(other, trusted)


if __name__ == "__main__":
    unittest.main()
