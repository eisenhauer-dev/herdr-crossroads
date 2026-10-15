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
            name = ARCHIVE.format("0.1.0")
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
            self.assertEqual(payloads["0.1.0"], payload)
            self.assertNotEqual(digest(payloads["0.1.0"]), digest(payloads["0.1.1"]))
            for version in ["0.1.0", "0.1.1"]:
                with tarfile.open(assets / ARCHIVE.format(version)) as package:
                    for filename in ["LICENSE", "THIRD-PARTY-NOTICES.html"]:
                        self.assertEqual(package.extractfile(filename).read(), (ROOT / filename).read_bytes())
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
            public_repo, public_assets, public_payloads = prepare_fixture(public_base, trusted, "0.1.0")
            self.assertEqual(public_payloads, {"0.1.0": payload})
            self.assertEqual(sorted(p.name for p in public_assets.iterdir()), [name, name + ".sha256"])
            self.assertEqual((public_repo / "herdr-plugin.toml").read_bytes(), (ROOT / "herdr-plugin.toml").read_bytes())
            self.assertFalse((public_repo / ".git").exists())
            (trusted / (name + ".sha256")).write_text("bad checksum\n")
            other = root / "rejected"
            other.mkdir()
            with self.assertRaises(AssertionError):
                prepare_fixture(other, trusted)


if __name__ == "__main__":
    unittest.main()
