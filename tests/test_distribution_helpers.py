"""Pure native-harness helper checks; does not launch Herdr or any subprocess."""

import io
from pathlib import Path
import tarfile
import tempfile
import tomllib
import unittest

from test_distribution import ARCHIVE, Screen, digest, fixture_next_version, prepare_fixture


class DistributionHelpers(unittest.TestCase):
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

    def test_artifacts_and_versions_stay_in_fixture(self):
        with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="xrd-helper-") as temporary:
            root = Path(temporary)
            trusted = root / "trusted"
            trusted.mkdir()
            payload = b"\x7fELF" + bytes(60)
            name = ARCHIVE.format("0.1.0")
            with tarfile.open(trusted / name, "w:gz") as package:
                for filename, data in [("herdr-crossroads", payload), ("herdr-crossroads.sha256",
                                       f"{digest(payload)}  herdr-crossroads\n".encode())]:
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
            fixture_next_version(repo)
            manifest = tomllib.loads((repo / "herdr-plugin.toml").read_text())
            self.assertEqual(manifest["version"], "0.1.1")
            self.assertEqual(manifest["panes"][0]["width"], "90%")
            self.assertEqual(tomllib.loads((repo / "Cargo.toml").read_text())["package"]["version"], "0.1.1")
            package = next(p for p in tomllib.loads((repo / "Cargo.lock").read_text())["package"]
                           if p["name"] == "herdr-crossroads")
            self.assertEqual(package["version"], "0.1.1")
            self.assertEqual((trusted / (name + ".sha256")).read_text(), checksum)
            (trusted / (name + ".sha256")).write_text("bad checksum\n")
            other = root / "rejected"
            other.mkdir()
            with self.assertRaises(AssertionError):
                prepare_fixture(other, trusted)


if __name__ == "__main__":
    unittest.main()
