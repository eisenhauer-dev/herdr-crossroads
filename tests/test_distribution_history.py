"""Portable historical distribution contracts; no public downloads or Herdr."""
import copy
import io
import json
from pathlib import Path
import socket
import tarfile
import tempfile
import unittest
from unittest.mock import Mock, patch

import test_distribution as distribution
import test_herdr_install
from terminal_screen import table_state


class HistoricalDistributionTests(unittest.TestCase):
    def check_cli_profile(self, trusted, name, version, profile, payload, notice, commit=None):
        commit = profile["commits"][0] if commit is None else commit

        def process(argv, **kwargs):
            stdout, stderr, status = "", "", 0
            if argv[1:] == ["--version"]:
                stdout = "herdr 0.9.5\n"
            elif argv[1] == "ls-remote":
                stdout = f"{commit}\trefs/tags/v{version}\n"
            elif argv[1:4] == ["plugin", "install", "herdr-crossroads"]:
                status, stderr = 2, "usage: install OWNER/REPO"
            elif argv[1:4] == ["plugin", "install", distribution.SOURCE]:
                env = kwargs["env"]
                # Fake only the Herdr boundary; exercise the real owned API
                # listener, registry/source guards and notice checks offline.
                with socket.socket(socket.AF_UNIX) as connection:
                    connection.settimeout(2)
                    connection.connect(env["HERDR_SOCKET_PATH"])
                    connection.sendall(b'{"id":"fixture","method":"plugin.list"}\n')
                    with connection.makefile("rb") as stream:
                        self.assertEqual(json.loads(stream.readline())["id"], "fixture")
                config = Path(env["XDG_CONFIG_HOME"]) / "herdr"
                managed = config / "managed"
                binary_dir = managed / "target/release"
                binary_dir.mkdir(parents=True)
                (managed / "herdr-plugin.toml").write_text(f'version = "{version}"\n')
                for filename, data, mode in [("herdr-crossroads", payload, 0o755),
                                             ("LICENSE", (distribution.ROOT / "LICENSE").read_bytes(), 0o644),
                                             (profile["notice"], notice, 0o644)]:
                    path = binary_dir / filename
                    path.write_bytes(data)
                    path.chmod(mode)
                (managed / profile["notice"]).write_bytes(notice)
                source = {"kind": "github", "owner": "eisenhauer-dev", "repo": "herdr-crossroads",
                          "requested_ref": commit, "resolved_commit": commit,
                          "installed_unix_ms": 1, "managed_path": str(managed)}
                (config / "plugins.json").write_text(json.dumps([{
                    "plugin_id": "herdr-crossroads", "version": version, "name": "Herdr Crossroads",
                    "min_herdr_version": "0.9.1", "enabled": True, "source": source,
                    "plugin_root": str(managed), "manifest_path": str(managed / "herdr-plugin.toml")}]))
                stdout = f"Installed herdr-crossroads from {distribution.SOURCE}."
            else:
                self.fail(f"unexpected command: {argv}")
            child = Mock(returncode=status)
            child.communicate.return_value = (stdout, stderr)
            return child

        with patch("subprocess.Popen", side_effect=process), patch("sys.stdout", io.StringIO()):
            test_herdr_install.run_case(Path("/not-executed/herdr"), trusted, name, "success",
                                       "v" + version, commit)

    def test_profiles_require_full_source_archive_binary_identity(self):
        for version, profile in distribution.HISTORICAL_RELEASES.items():
            for commit in profile["commits"]:
                self.assertIs(distribution.historical_profile(
                    version, commit, profile["archive"], profile["binary"]), profile)
            for commit, archive, binary in [
                    ("a" * 40, profile["archive"], profile["binary"]),
                    (None, profile["archive"], profile["binary"]),
                    (profile["commits"][0], "0" * 64, profile["binary"]),
                    (profile["commits"][0], profile["archive"], "0" * 64)]:
                with self.subTest(version=version, commit=commit, archive=archive, binary=binary):
                    with self.assertRaisesRegex(AssertionError, "identity"):
                        distribution.historical_profile(version, commit, archive, binary)
        self.assertIsNone(distribution.historical_profile("0.3.0", "a" * 40, "b" * 64, "c" * 64))
        self.assertIsNone(distribution.historical_profile("unknown", None, "b" * 64, "c" * 64))

    def test_historical_fixture_and_managed_notices_ignore_current_inventory(self):
        # Small synthetic archives exercise the real hash gates, not public proof.
        for version in ["0.0.1", "0.2.0"]:
            for mutation in [None, "notice", "license", "identity", "member", "checksum"]:
                with self.subTest(version=version, mutation=mutation), tempfile.TemporaryDirectory(
                        dir="/tmp/opencode", prefix="xrd-history-helper-") as temporary:
                    base = Path(temporary)
                    trusted, fixture = base / "trusted", base / "fixture"
                    trusted.mkdir()
                    fixture.mkdir()
                    profile = copy.deepcopy(distribution.HISTORICAL_RELEASES[version])
                    notice = b"Immutable historical notice\n"
                    payload = b"\x7fELF" + bytes(60)
                    profile["binary"] = distribution.digest(payload)
                    name = distribution.ARCHIVE.format(version)
                    with tarfile.open(trusted / name, "w:gz") as archive:
                        members = [("herdr-crossroads", payload), ("herdr-crossroads.sha256",
                                    f"{distribution.digest(payload)}  herdr-crossroads\n".encode()),
                                   ("LICENSE", (distribution.ROOT / "LICENSE").read_bytes()),
                                   (profile["notice"], notice)]
                        if mutation == "member":
                            members.append(("extra", b"unexpected"))
                        for filename, data in members:
                            if mutation == "notice" and filename == profile["notice"]:
                                data += b"tampered"
                            if mutation == "license" and filename == "LICENSE":
                                data += b"tampered"
                            entry = tarfile.TarInfo(filename)
                            entry.size = len(data)
                            archive.addfile(entry, io.BytesIO(data))
                    profile["archive"] = distribution.digest((trusted / name).read_bytes())
                    (trusted / (name + ".sha256")).write_text(
                        f"{profile['archive']}  {name}\n" if mutation != "checksum" else "invalid\n")
                    commits = {version: profile["commits"][0] if mutation != "identity" else "a" * 40}
                    with patch.dict(distribution.HISTORICAL_RELEASES, {version: profile}), patch.dict(
                            distribution.INSTALLER, LEGACY_NOTICE_SHA256=distribution.digest(notice),
                            LEGACY_MARKDOWN_SHA256=distribution.digest(notice)):
                        if mutation:
                            with self.assertRaises(AssertionError):
                                distribution.prepare_fixture(fixture, trusted, version, public_commits=commits)
                            continue
                        repo, assets, payloads = distribution.prepare_fixture(
                            fixture, trusted, version, public_commits=commits)
                        self.assertEqual(payloads, {version: payload})
                        self.assertEqual((assets / name).read_bytes(), (trusted / name).read_bytes())
                        self.assertNotEqual((repo / "THIRD-PARTY-NOTICES.md").read_bytes(), notice)
                        managed = base / "managed"
                        managed.mkdir()
                        (managed / profile["notice"]).write_bytes(notice)
                        expected = distribution.expected_notices(managed, repo, True, profile)
                        self.assertEqual(expected[profile["notice"]], notice)
                        self.check_cli_profile(trusted, name, version, profile, payload, notice)
                        with self.assertRaisesRegex(AssertionError, "identity"):
                            self.check_cli_profile(trusted, name, version, profile, payload, notice, "a" * 40)
                        (managed / profile["notice"]).write_bytes(notice + b"tampered")
                        with self.assertRaisesRegex(AssertionError, "notice hash"):
                            distribution.expected_notices(managed, repo, True, profile)
                        (managed / profile["notice"]).write_bytes(notice)
                        other = ("THIRD-PARTY-NOTICES.md" if profile["notice"].endswith(".html")
                                 else "THIRD-PARTY-NOTICES.html")
                        (managed / other).write_bytes(notice)
                        with self.assertRaises(AssertionError):
                            distribution.expected_notices(managed, repo, True, profile)

    def test_current_inventory_cannot_use_historical_notice_exception(self):
        with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="xrd-current-notice-") as temporary:
            root = Path(temporary)
            trusted = root / "trusted"
            trusted.mkdir()
            payload = b"\x7fELF" + bytes(60)
            name = distribution.ARCHIVE.format("0.0.1")
            for notice in [(distribution.ROOT / "THIRD-PARTY-NOTICES.md").read_bytes(),
                           b"tampered current inventory", b"historical inventory"]:
                with tarfile.open(trusted / name, "w:gz") as archive:
                    for filename, data in [("herdr-crossroads", payload), ("herdr-crossroads.sha256",
                                            f"{distribution.digest(payload)}  herdr-crossroads\n".encode()),
                                           ("LICENSE", (distribution.ROOT / "LICENSE").read_bytes()),
                                           ("THIRD-PARTY-NOTICES.md", notice)]:
                        entry = tarfile.TarInfo(filename)
                        entry.size = len(data)
                        archive.addfile(entry, io.BytesIO(data))
                (trusted / (name + ".sha256")).write_text(
                    f"{distribution.digest((trusted / name).read_bytes())}  {name}\n")
                with tempfile.TemporaryDirectory(dir=root) as fixture:
                    if notice == (distribution.ROOT / "THIRD-PARTY-NOTICES.md").read_bytes():
                        distribution.prepare_fixture(Path(fixture), trusted)
                    else:
                        with self.assertRaises(AssertionError):
                            distribution.prepare_fixture(Path(fixture), trusted)
                # Neither a public flag nor an old version opts unknown bytes in.
                if notice == b"historical inventory":
                    with tempfile.TemporaryDirectory(dir=root) as fixture, patch.dict(
                            distribution.INSTALLER, LEGACY_MARKDOWN_SHA256=distribution.digest(notice)):
                        with self.assertRaises(AssertionError):
                            distribution.prepare_fixture(Path(fixture), trusted, "0.0.1",
                                                         public_commits={"0.0.1": "a" * 40})

    def test_historical_slash_search_enter_filter_and_reopen(self):
        profile = distribution.HISTORICAL_RELEASES["0.0.1"]
        screen = distribution.Screen(rows=25, cols=120)

        def paint(query, badge, rows, editing=False):
            # Exact prompt/badges from historical presentation/footer.rs; host
            # chrome to the right and labels below must not enter the popup crop.
            screen.feed(("\x1b[2J\x1b[2;5HSearch: " + query.ljust(30) + badge
                         + "    HOST\x1b[4;5H" + rows
                         + ("\x1b[16;5HEnter/Esc: select  Backspace: erase" if editing else "")
                         + "\x1b[25;1HOrigin").encode())

        paint("/ to search", "Select", "Origin\x1b[5;5HArrival")
        self.assertEqual(screen.popup_view(), "")
        self.assertIn("Origin", distribution.popup_view(screen, profile))
        self.assertNotIn("HOST", distribution.popup_view(screen, profile))
        self.assertEqual(distribution.search_input(profile), b"/Arrival")
        paint("Arrival", "Search", "Arrival", True)
        self.assertTrue(distribution.filtered_popup(screen, True, profile))
        self.assertFalse(distribution.filtered_popup(screen, False, profile))
        self.assertFalse(distribution.filtered_popup(screen, True))
        paint("Arrival", "Select", "Arrival")
        self.assertTrue(distribution.filtered_popup(screen, False, profile))
        self.assertNotIn("Origin", distribution.popup_view(screen, profile))
        paint("/ to search", "Select", "Origin\x1b[5;5HArrival")
        self.assertIn("Search: / to search", distribution.popup_view(screen, profile))
        self.assertIn("Origin", distribution.popup_view(screen, profile))
        paint("Arrival", "Help", "Arrival")
        self.assertEqual(distribution.popup_view(screen, profile), "")
        paint("/ to search", "Select", "Origin\x1b[5;5HArrival")
        screen.feed(b"\x1b[18;5HSearch: / to search               Select")
        self.assertEqual(distribution.popup_view(screen, profile), "")
        with self.assertRaisesRegex(AssertionError, "unknown popup profile"):
            screen.popup_view(profile="auto")

    def test_imported_current_table_reader_never_accepts_historical_badges(self):
        screen = distribution.Screen(rows=10, cols=100)
        table = ("\x1b[3;1H    Agent/Pane Name        Status"
                 "\x1b[4;1H    -------------------"
                 "\x1b[5;1H  > Arrival"
                 "\x1b[6;1H  ─────────────────────")
        for badge in ["Select", "Search", "MODE: SELECT"]:
            screen.feed(("\x1b[2J\x1b[1;1H  Search: Arrival     " + badge + table).encode())
            if badge == "MODE: SELECT":
                state = table_state(screen)
                self.assertEqual((state["mode"], state["query"], state["panes"], state["selected"]),
                                 ("Select", "Arrival", ("Arrival",), (0, "Arrival")))
            else:
                self.assertIsNone(table_state(screen))

    def test_current_badge_f_search_and_footer_remain_required(self):
        screen = distribution.Screen(rows=20, cols=100)
        self.assertEqual(distribution.search_input(), b"fArrival")
        screen.feed(b"\x1b[2;5HSearch: Arrival         MODE: SELECT\x1b[4;5HArrival"
                    b"\x1b[16;5HEnter/Esc: select  Backspace: erase")
        self.assertTrue(distribution.filtered_popup(screen, True))
        self.assertFalse(distribution.filtered_popup(screen, False))
        self.assertFalse(distribution.filtered_popup(screen, True, distribution.HISTORICAL_RELEASES["0.0.1"]))
        screen.feed(b"\x1b[16;1H\x1b[2K")
        self.assertTrue(distribution.filtered_popup(screen, False))
        screen.feed(b"\x1b[2J\x1b[2;5HSearch: Arrival         Select\x1b[4;5HArrival")
        self.assertEqual(screen.popup_view(), "")
        self.assertFalse(distribution.filtered_popup(screen, False))


if __name__ == "__main__":
    unittest.main()
