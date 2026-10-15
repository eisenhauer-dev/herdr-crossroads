"""Notice reproduction from the offline graph and installed Rust notice source."""

import html
from pathlib import Path
import runpy
import subprocess
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]


class NoticeTests(unittest.TestCase):
    def test_verified_glibc_startup_headers_are_delivered(self):
        path = ROOT / "LICENSES/glibc-2.43-2ubuntu2.4-startup.txt"
        self.assertTrue(path.is_file(), "exact glibc startup exception headers must be checked in")
        text = path.read_text()
        for source in ["sysdeps/x86_64/start.S", "sysdeps/x86_64/crti.S", "sysdeps/x86_64/crtn.S",
                       "csu/abi-note.c", "stdlib/atexit.c", "stdlib/at_quick_exit.c",
                       "sysdeps/pthread/pthread_atfork.c", "debug/stack_chk_fail_local.c"]:
            self.assertIn("File: " + source + "\n", text)
        self.assertEqual(text.count("gives you unlimited"), 8)
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        self.assertTrue(html.escape(text) in generator["generate"](),
                        "full verified startup header artifact must accompany the binary notices")

    def test_glibc_startup_exception_evidence_requires_matching_source_package(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        original = generator["command"]

        def command(*args):
            if args[:3] == ("dpkg-query", "-W", "-f=${source:Package} ${source:Version}"):
                return "glibc 2.44-1"
            return original(*args)

        with patch.dict(generator["system_notices"].__globals__, command=command):
            with self.assertRaisesRegex(ValueError, "glibc startup source evidence"):
                generator["system_notices"]()

    def test_ambiguous_system_object_ownership_is_rejected(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        original = generator["command"]

        def command(*args):
            if args[:2] == ("dpkg-query", "-S"):
                return "libc6-dev:amd64: /object\nother-package: /object"
            return original(*args)

        with patch.dict(generator["system_notices"].__globals__, command=command):
            with self.assertRaisesRegex(ValueError, "ambiguous package ownership"):
                generator["system_notices"]()

    def test_system_package_version_drift_changes_generated_inventory(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        original = generator["command"]
        baseline = generator["system_notices"]()

        def command(*args):
            result = original(*args)
            return result + "+rebuilt" if (args[:2] == ("dpkg-query", "-W")
                                           and "${binary:Package}" in args[2]) else result

        with patch.dict(generator["system_notices"].__globals__, command=command):
            self.assertNotEqual(generator["system_notices"](), baseline)

    def test_system_notices_preserve_installed_copyright_and_full_licenses(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        generated = generator["generate"]()
        self.assertTrue("<h1>System startup and runtime provenance</h1>" in generated,
                        "binary notice inventory must include system startup/runtime evidence")
        for filename in ["/usr/share/doc/libc6-dev/copyright",
                         "/usr/share/doc/libgcc-s1/copyright",
                         "/usr/share/common-licenses/LGPL-2.1",
                         "/usr/share/common-licenses/GPL-3"]:
            self.assertTrue(html.escape(Path(filename).read_text()) in generated,
                            f"full installed notice/license must be preserved: {filename}")
        self.assertTrue("Notice/license file SHA256:" in generated,
                        "copied system notices need hash and installed provider provenance")
        for filename in ["Scrt1.o", "crti.o", "crtn.o", "crtbeginS.o", "crtendS.o", "libgcc.a"]:
            path = Path(subprocess.check_output(["gcc", f"-print-file-name={filename}"], text=True).strip()).resolve()
            self.assertTrue(str(path) in generated, f"missing startup/runtime provenance: {filename}")

    def test_checked_in_notice_reproduces_all_exact_cached_sources(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        generated = generator["generate"]()
        self.assertEqual((ROOT / "THIRD-PARTY-NOTICES.html").read_text(), generated)
        self.assertIn("78 unique registry package versions", generated)
        for text in ["foldhash 0.2.0", "unicode-ident 1.0.26", "LICENSE-UNICODE",
                     "unicode-general-category 1.1.0", "<h3>COPYING</h3>",
                     "Copyright notices for The Rust Standard Library"]:
            self.assertIn(text, generated)


if __name__ == "__main__":
    unittest.main()
