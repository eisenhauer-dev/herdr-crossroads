"""Notice reproduction from the offline graph and installed Rust notice source."""

import html
import copy
import json
import hashlib
from html.parser import HTMLParser
from pathlib import Path, PureWindowsPath
import runpy
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]


class NoticeTests(unittest.TestCase):
    def test_windows_relative_paths_preserve_reviewed_system_notice_suffix(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        system_notices = generator["system_notices"]
        baseline = system_notices(validate_installed=False)

        class WindowsRelativePath(type(ROOT)):
            # Keep real filesystem reads native; serialize actual Windows paths.
            def relative_to(self, *other):
                return PureWindowsPath(super().relative_to(*other))

        windows_root = WindowsRelativePath(ROOT)
        relative = (windows_root / "LICENSES/ubuntu-24.04-system.json").relative_to(windows_root)
        self.assertIsInstance(relative, PureWindowsPath)
        self.assertIn("\\", str(relative))
        with patch.dict(system_notices.__globals__, ROOT=windows_root):
            output = system_notices(validate_installed=False)
        self.assertEqual(output, baseline)
        suffix = generator["markdown"]("\n".join(output))
        self.assertTrue((ROOT / "THIRD-PARTY-NOTICES.md").read_bytes().decode().endswith(suffix))

    def test_published_markdown_installs_with_windows_expanded_checkout(self):
        from test_install_binary import InstallBinaryTests
        fixture = InstallBinaryTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.check_published_markdown_with_windows_expanded_checkout()

    def test_autocrlf_checkout_preserves_pinned_notice_evidence(self):
        evidence = sorted((ROOT / "LICENSES").glob("*-system.json"))
        evidence += sorted((ROOT / "LICENSES").glob("glibc-*-startup.txt"))
        self.assertEqual(len(evidence), 4)
        with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-notice-checkout-") as temporary:
            repo = Path(temporary)

            def git(*args):
                return subprocess.run(["git", "-c", "core.autocrlf=true", *args], cwd=repo,
                                      check=True, capture_output=True)

            git("init")
            (repo / ".gitattributes").write_bytes((ROOT / ".gitattributes").read_bytes())
            (repo / "LICENSES").mkdir()
            inputs = [ROOT / "THIRD-PARTY-NOTICES.md", *evidence]
            for source in inputs:
                (repo / source.relative_to(ROOT)).write_bytes(source.read_bytes())
            (repo / "ordinary.txt").write_bytes(b"ordinary text\n")
            git("add", ".")
            # Materialize the real index through Git's Windows-style checkout
            # conversion, without changing any settings or files in the real repo.
            for source in inputs:
                (repo / source.relative_to(ROOT)).unlink()
            (repo / "ordinary.txt").unlink()
            git("checkout-index", "--all", "--force")
            self.assertEqual((repo / "ordinary.txt").read_bytes(), b"ordinary text\r\n")
            generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
            system_notices = generator["system_notices"]
            baseline = system_notices(validate_installed=False)
            with patch.dict(system_notices.__globals__, ROOT=repo):
                self.assertEqual(system_notices(validate_installed=False), baseline)
                for source in inputs:
                    path = repo / source.relative_to(ROOT)
                    self.assertEqual(path.read_bytes(), source.read_bytes(), str(source))
                for source in evidence:
                    path = repo / source.relative_to(ROOT)
                    original = path.read_bytes()
                    path.write_bytes(original.replace(b"\n", b"\r\n"))
                    with self.subTest(source=source):
                        if source.suffix == ".json":
                            self.assertNotEqual(system_notices(validate_installed=False), baseline)
                        else:
                            with self.assertRaisesRegex(ValueError, "startup source evidence"):
                                system_notices(validate_installed=False)
                    path.write_bytes(original)

    def test_windows_crates_have_explicit_verbatim_notices(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        generated = generator["generate"]()
        for name, version in [("crossterm_winapi", "0.9.1"), ("winapi", "0.3.9"), ("windows-link", "0.2.1")]:
            heading = generator["markdown"](f"<h2>{name} {version}</h2>").strip()
            self.assertIn("\n" + heading + "\n", generated)
        metadata = json.loads(generator["command"](
            "cargo", "metadata", "--offline", "--locked", "--format-version", "1",
            "--filter-platform", "x86_64-pc-windows-msvc"))
        count = 0
        for package in metadata["packages"]:
            if package["name"] not in {"crossterm_winapi", "winapi", "windows-link"}:
                continue
            heading = generator["markdown"](f"<h2>{package['name']} {package['version']}</h2>").strip()
            section = generated.split("\n" + heading + "\n", 1)[1].split("\n## ", 1)[0]
            for path in Path(package["manifest_path"]).parent.rglob("*"):
                if path.is_file() and re.search(r"licen[cs]e|notice|copyright|copying|attribution", path.name, re.I):
                    self.assertIn("\n" + path.read_bytes().decode() + "\n", section, str(path))
                    count += 1
        self.assertEqual(count, 5)

    def test_darwin_graphs_are_covered_by_verbatim_checked_in_inventory(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        check = generator.get("check_darwin")
        self.assertIsNotNone(check, "Darwin requires an explicit licensing gate, not Linux gcc/dpkg")
        for target in ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-pc-windows-msvc"]:
            with patch.dict(check.__globals__, installed_system=lambda: self.fail("Darwin must not use Linux gcc/dpkg")):
                check(target)

    def test_darwin_notice_gate_rejects_missing_attribution_unknown_graph_or_rust_drift(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        check = generator["check_darwin"]
        original = generator["command"]
        for failure in ["crate", "rust", "graph"]:
            def command(*args):
                result = original(*args)
                if failure == "rust" and args == ("rustc", "--version"):
                    return result.replace("1.99.0", "1.99.1")
                if failure == "graph" and args[:2] == ("cargo", "tree"):
                    return result + "\nunreviewed-crate v1.0.0\n"
                if failure == "crate" and args[:2] == ("cargo", "metadata"):
                    data = json.loads(result)
                    for package in data["packages"]:
                        if package["name"] == "serde_json":
                            package["version"] = "0.0.0"
                    return json.dumps(data)
                return result
            for target in ["aarch64-apple-darwin", "x86_64-pc-windows-msvc"]:
                with self.subTest(failure=failure, target=target), patch.dict(check.__globals__, command=command):
                    with self.assertRaises(ValueError):
                        check(target)
        with self.assertRaisesRegex(ValueError, "unsupported"):
            check("unknown-apple-darwin")

    def test_darwin_cli_runs_explicit_target_gate(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        with patch.dict(generator["main"].__globals__, check_darwin=lambda target: self.assertEqual(target, "aarch64-apple-darwin")):
            with patch.object(sys, "argv", ["third-party-notices.py", "--check", "--target", "aarch64-apple-darwin"]):
                generator["main"]()

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
        generated = generator["generate"](windows=False)
        self.assertIn("78 unique registry package versions", generated)
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
        targets = [generator["TARGET"], "x86_64-pc-windows-msvc"]
        active = set()
        for target in targets:
            tree = generator["command"]("cargo", "tree", "--offline", "--locked", "--target",
                                        target, "-e", "normal,build", "--prefix", "none")
            active.update(re.findall(r"^([\w-]+) v([^\s]+)", tree, re.M))
        self.assertEqual(len(active) - 1, 81)
        self.assertIn(f"{len(active) - 1} unique registry package versions", generated)
        for text in ["foldhash 0.2.0", "unicode-ident 1.0.26", "LICENSE-UNICODE",
                     "unicode-general-category 1.1.0", "### COPYING",
                     "Copyright notices for The Rust Standard Library"]:
            self.assertIn(text, generated)


if __name__ == "__main__":
    unittest.main()
