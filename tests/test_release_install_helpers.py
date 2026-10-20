"""Fixture and isolation regressions for the opt-in native installation gate."""

import io
from pathlib import Path
import runpy
import tarfile
import unittest


HELPERS = runpy.run_path(str(Path(__file__).with_name("test_release_install.py")))


class ReleaseInstallHelpersTests(unittest.TestCase):
    def test_checksum_contract(self):
        self.assertEqual(HELPERS["checksum"](b"", "binary"),
                         b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  binary\n")

    def test_rejection_fixtures_retain_requested_types_paths_and_payloads(self):
        entries = [("herdr-crossroads", b"executable", tarfile.REGTYPE),
                   ("../escape", b"unsafe", tarfile.REGTYPE),
                   ("symbolic", b"", tarfile.SYMTYPE),
                   ("hard", b"", tarfile.LNKTYPE)]
        with tarfile.open(fileobj=io.BytesIO(HELPERS["pack"](entries)), mode="r:gz") as archive:
            self.assertEqual([(m.name, m.type) for m in archive.getmembers()],
                             [(name, kind) for name, _, kind in entries])
            self.assertEqual(archive.extractfile("herdr-crossroads").read(), b"executable")
            self.assertEqual(archive.getmember("herdr-crossroads").mode, 0o755)
            self.assertEqual(archive.getmember("symbolic").linkname, "herdr-crossroads")
            self.assertEqual(archive.getmember("hard").linkname, "herdr-crossroads")


if __name__ == "__main__":
    unittest.main()
