"""Notice reproduction from the offline graph and installed Rust notice source."""

import html
import copy
import json
from pathlib import Path
import runpy
import subprocess
import sys
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]


class NoticeTests(unittest.TestCase):
    def test_check_rejects_unreviewed_builder_even_when_cargo_and_rust_match(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        original = generator["command"]

        def command(*args):
            result = original(*args)
            return result + "+rebuilt" if (args[:2] == ("dpkg-query", "-W")
                                           and "${Version}" in args[2]) else result

        with patch.dict(generator["main"].__globals__, command=command):
            with patch.object(sys, "argv", ["third-party-notices.py", "--check"]):
                with self.assertRaisesRegex(ValueError, "reviewed system builder"):
                    generator["main"]()

    def test_reviewed_profiles_require_untampered_sources_and_notice_bytes(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        original = Path.read_bytes
        targets = ["LICENSES/ubuntu-24.04-system.json",
                   "LICENSES/local-glibc-2.43-system.json",
                   "LICENSES/glibc-2.39-0ubuntu8.9-startup.txt",
                   "LICENSES/glibc-2.43-2ubuntu2.4-startup.txt"]
        for target in targets:
            for missing in [False, True]:
                def read(path):
                    if path == ROOT / target:
                        if missing:
                            raise FileNotFoundError(target)
                        data = original(path)
                        if target.endswith(".json"):
                            profile = json.loads(data)
                            profile["notices"][0]["text"] += "tampered"
                            return json.dumps(profile).encode()
                        return data + b"tampered"
                    return original(path)
                with self.subTest(target=target, missing=missing), patch.object(Path, "read_bytes", read):
                    with self.assertRaises((ValueError, FileNotFoundError)):
                        generator["system_notices"]()

    def test_both_reviewed_builders_match_but_unknown_or_incomplete_inputs_fail(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        baseline = generator["system_notices"]()
        for name in ["local-glibc-2.43", "ubuntu-24.04"]:
            profile = json.loads((ROOT / f"LICENSES/{name}-system.json").read_text())
            with patch.dict(generator["system_notices"].__globals__,
                            installed_system=lambda: (profile["candidates"], profile["notices"])):
                output = generator["system_notices"]()
                self.assertEqual(output, baseline)
                self.assertIn(profile["label"], "\n".join(output))
            for field in ["resolved", "sha256", "packages"]:
                candidates = copy.deepcopy(profile["candidates"])
                candidates[0][field] = "different"
                with patch.dict(generator["system_notices"].__globals__,
                                installed_system=lambda: (candidates, profile["notices"])):
                    with self.assertRaisesRegex(ValueError, "reviewed system builder"):
                        generator["system_notices"]()
            with patch.dict(generator["system_notices"].__globals__,
                            installed_system=lambda: (profile["candidates"][:-1], profile["notices"])):
                with self.assertRaisesRegex(ValueError, "reviewed system builder"):
                    generator["system_notices"]()
            notices = copy.deepcopy(profile["notices"])
            notices[0]["text"] += "changed installed license"
            with patch.dict(generator["system_notices"].__globals__,
                            installed_system=lambda: (profile["candidates"], notices)):
                with self.assertRaisesRegex(ValueError, "reviewed system builder"):
                    generator["system_notices"]()

    def test_verified_glibc_startup_headers_are_delivered(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        generated = generator["generate"]()
        for version in ["2.43-2ubuntu2.4", "2.39-0ubuntu8.9"]:
            path = ROOT / f"LICENSES/glibc-{version}-startup.txt"
            self.assertTrue(path.is_file(), "exact glibc startup exception headers must be checked in")
            text = path.read_text()
            for source in ["sysdeps/x86_64/start.S", "sysdeps/x86_64/crti.S", "sysdeps/x86_64/crtn.S",
                           "csu/abi-note.c", "stdlib/atexit.c", "stdlib/at_quick_exit.c",
                           "sysdeps/pthread/pthread_atfork.c", "debug/stack_chk_fail_local.c"]:
                self.assertIn("File: " + source + "\n", text)
            self.assertEqual(text.count("gives you unlimited"), 8)
            self.assertTrue(html.escape(text) in generated,
                            "full verified startup header artifact must accompany the binary notices")

    def test_glibc_startup_exception_evidence_requires_matching_source_package(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        original = generator["command"]

        def command(*args):
            if args[:3] == ("dpkg-query", "-W", "-f=${Version} ${source:Package} ${source:Version}"):
                return "2.44-1 glibc 2.44-1"
            return original(*args)

        with patch.dict(generator["system_notices"].__globals__, command=command):
            with self.assertRaisesRegex(ValueError, "reviewed system builder"):
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

    def test_system_package_version_drift_requires_review_not_automatic_regeneration(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        original = generator["command"]

        def command(*args):
            result = original(*args)
            return result + "+rebuilt" if (args[:2] == ("dpkg-query", "-W")
                                           and "${Version}" in args[2]) else result

        with patch.dict(generator["system_notices"].__globals__, command=command):
            with self.assertRaisesRegex(ValueError, "reviewed system builder"):
                generator["system_notices"]()

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
