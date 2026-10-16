"""Notice reproduction from the offline graph and installed Rust notice source."""

import html
import copy
import json
import hashlib
from html.parser import HTMLParser
from pathlib import Path
import runpy
import re
import subprocess
import sys
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]


class NoticeTests(unittest.TestCase):
    def test_published_html_requires_exact_version_and_equivalent_trusted_markdown(self):
        from test_install_binary import InstallBinaryTests
        fixture = InstallBinaryTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.check_published_html_requires_exact_version_and_equivalent_trusted_markdown()

    def test_cached_license_bytes_keep_original_line_endings(self):
        self.assertIn("THIRD-PARTY-NOTICES.md -text", (ROOT / ".gitattributes").read_text().splitlines())
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        generated = generator["generate"]()
        tree = generator["command"]("cargo", "tree", "--offline", "--locked", "--target",
                                    generator["TARGET"], "-e", "normal,build", "--prefix", "none")
        active = set(re.findall(r"^([\w-]+) v([^\s]+)", tree, re.M))
        metadata = json.loads(generator["command"](
            "cargo", "metadata", "--offline", "--locked", "--format-version", "1",
            "--filter-platform", generator["TARGET"]))
        count = 0
        for package in metadata["packages"]:
            if not package["source"] or (package["name"], package["version"]) not in active:
                continue
            for path in Path(package["manifest_path"]).parent.rglob("*"):
                if path.is_file() and re.search(r"licen[cs]e|notice|copyright|copying|attribution", path.name, re.I):
                    self.assertIn("\n" + path.read_bytes().decode() + "\n", generated, str(path))
                    count += 1
        self.assertEqual(count, 140)

    def test_output_is_markdown_with_every_original_verbatim_block(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        generated = generator["generate"]()
        self.assertTrue(generated.startswith("# Crossroads third-party notices\n"),
                        "notices must be genuine Markdown, not renamed HTML")
        original = generator["generate_html"]()
        self.assertEqual(hashlib.sha256(original.replace("\r\n", "\n").encode()).hexdigest(),
                         "08b1ee55eea459afb4e0fe55c68ce91c2f1f815df9d1b9505ffecfdc9226b174")
        class Blocks(HTMLParser):
            def __init__(self):
                super().__init__()
                self.blocks = []
                self.current = None
                self.prose = []
                self.urls = []
            def handle_starttag(self, tag, attrs):
                if tag == "pre":
                    self.current = ""
                if tag == "a":
                    self.urls.append(dict(attrs)["href"])
            def handle_data(self, data):
                if self.current is not None:
                    self.current += data
                elif data.strip():
                    self.prose.append(re.sub(r"\s+", " ", data).strip())
            def handle_endtag(self, tag):
                if tag == "pre":
                    self.blocks.append(self.current)
                    self.current = None
        parser = Blocks()
        parser.feed(original)
        self.assertEqual(len(parser.blocks), 458)
        # Delimiters add one newline; the enclosed bytes, including all original
        # leading/trailing whitespace and decoded entities, remain untouched.
        self.assertEqual(re.findall(r"^(`{3,})\n(.*?)\n\1$", generated, re.M | re.S),
                         [("`" * max(3, max((len(s) + 1 for s in re.findall(r"`+", block)), default=0)), block)
                          for block in parser.blocks])
        for text in parser.prose:
            text = text.replace("Texts below are reproduced without modification (HTML-escaped).",
                                "Texts below are reproduced without modification in verbatim fenced blocks.")
            escaped = re.sub(r"([\\`*_{}\[\]<>])", r"\\\1", text)
            self.assertIn(escaped, generated)
        for url in parser.urls:
            if not url.startswith("#"):
                self.assertIn("](<" + url + ">)", generated)
        self.assertEqual(len(parser.urls), 148)
        self.assertNotIn("<details>", generated)
        self.assertNotIn("<pre>", generated)
        self.assertNotIn("<html>", generated)
        self.assertIn("[Short version for non-lawyers](<#short-version-for-non-lawyers>)", generated)

    def test_conversion_preserves_entities_whitespace_and_fence_collisions(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        convert = generator["markdown"]
        text = "\r\n  A & B <tag> &#34; literal\t\n```\n````\nend  "
        result = convert("<h1>Notices</h1><pre>" + html.escape(text) + "</pre>")
        self.assertEqual(result, "# Notices\n\n`````\n" + text + "\n`````\n")
        with self.assertRaisesRegex(ValueError, "unsupported"):
            convert("<script>lost attribution</script>")
        self.assertEqual(convert("<div><p>parent</p><div><p>exception</p></div><p>parent again</p></div>"),
                         "> parent\n\n> > exception\n\n> parent again\n")

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
            self.assertTrue(text in generated,
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
        self.assertTrue("# System startup and runtime provenance" in generated,
                        "binary notice inventory must include system startup/runtime evidence")
        for filename in ["/usr/share/doc/libc6-dev/copyright",
                         "/usr/share/doc/libgcc-s1/copyright",
                         "/usr/share/common-licenses/LGPL-2.1",
                         "/usr/share/common-licenses/GPL-3"]:
            self.assertTrue(Path(filename).read_text() in generated,
                            f"full installed notice/license must be preserved: {filename}")
        self.assertTrue("Notice/license file SHA256:" in generated,
                        "copied system notices need hash and installed provider provenance")
        for filename in ["Scrt1.o", "crti.o", "crtn.o", "crtbeginS.o", "crtendS.o", "libgcc.a"]:
            path = Path(subprocess.check_output(["gcc", f"-print-file-name={filename}"], text=True).strip()).resolve()
            self.assertTrue(str(path) in generated, f"missing startup/runtime provenance: {filename}")

    def test_checked_in_notice_reproduces_all_exact_cached_sources(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        generated = generator["generate"]()
        self.assertEqual((ROOT / "THIRD-PARTY-NOTICES.md").read_bytes().decode(), generated)
        self.assertIn("78 unique registry package versions", generated)
        for text in ["foldhash 0.2.0", "unicode-ident 1.0.26", "LICENSE-UNICODE",
                     "unicode-general-category 1.1.0", "### COPYING",
                     "Copyright notices for The Rust Standard Library"]:
            self.assertIn(text, generated)


if __name__ == "__main__":
    unittest.main()
