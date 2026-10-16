"""Installer tests: synthetic ELF and mocked HTTP, no network, Herdr or Rust.

The full historical notice fixture is exercised separately by test_notices.py
on builders with the cached sources and Rust documentation installed.
"""

import hashlib
import io
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unittest
from unittest.mock import patch
import urllib.error


ROOT = Path(__file__).resolve().parents[1]
ARCHIVE = "herdr-crossroads-0.0.1-x86_64-unknown-linux-gnu.tar.gz"
# Header-only fixture is intentionally not a runnable binary.
ELF = b"\x7fELF\x02\x01\x01" + bytes(9) + b"\x03\x00\x3e\x00\x01\x00\x00\x00" + bytes(40)


class InstallBinaryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-install-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.repo = self.base / "repo"
        (self.repo / "scripts").mkdir(parents=True)
        for name in ["install-binary.py", "check-version.py"]:
            source = ROOT / "scripts" / name
            if source.exists():
                shutil.copy2(source, self.repo / "scripts" / name)
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml", "LICENSE", "THIRD-PARTY-NOTICES.md"]:
            if not (ROOT / name).exists():
                continue
            shutil.copy2(ROOT / name, self.repo / name)
        # Asset expectations use a fixed fixture version, not the checkout version.
        version = tomllib.loads((self.repo / "Cargo.toml").read_text())["package"]["version"]
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            path = self.repo / name
            prefix = 'name = "herdr-crossroads"\n' if name == "Cargo.lock" else ""
            path.write_text(path.read_text().replace(
                prefix + f'version = "{version}"', prefix + 'version = "0.0.1"', 1))
        self.assets = self.base / "assets"
        self.assets.mkdir()
        self.binary = self.repo / "target/release/herdr-crossroads"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b"previous binary")
        self.binary.chmod(0o751)
        self.env = {"PATH": "", "CROSSROADS_ASSET_DIR": str(self.assets)}
        self.package()

    def package(self, data=ELF, members=None, internal=None):
        checksum = internal if internal is not None else f"{hashlib.sha256(data).hexdigest()}  herdr-crossroads\n".encode()
        if members is None:
            members = [("herdr-crossroads", data, tarfile.REGTYPE),
                       ("herdr-crossroads.sha256", checksum, tarfile.REGTYPE)]
            members += [(name, (self.repo / name).read_bytes() if (self.repo / name).exists()
                         else b"missing notices", tarfile.REGTYPE)
                         for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]]
        archive = self.assets / ARCHIVE
        with tarfile.open(archive, "w:gz") as package:
            for name, contents, kind in members:
                entry = tarfile.TarInfo(name)
                entry.type = kind
                entry.mode = 0o755
                entry.linkname = "../../escape"
                entry.size = len(contents) if kind in [tarfile.REGTYPE, tarfile.CONTTYPE] else 0
                package.addfile(entry, io.BytesIO(contents))
        self.archive_checksum()

    def archive_checksum(self):
        archive = self.assets / ARCHIVE
        (self.assets / (ARCHIVE + ".sha256")).write_text(
            f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {ARCHIVE}\n")

    def install(self, patch=""):
        helper = self.repo / "scripts/install-binary.py"
        # Before implementation, no installer means no domain effect. Assertions
        # below observe installation/rejection, not a missing-file setup failure.
        code = ("import platform, runpy; " + patch +
                f"runpy.run_path({str(helper)!r}, run_name='__main__')") if helper.exists() else "pass"
        return subprocess.run([sys.executable, "-c", code], cwd=self.base,
                              env=self.env, text=True, capture_output=True)

    def reject(self, patch=""):
        result = self.install(patch)
        self.assertNotEqual(result.returncode, 0, "invalid local asset must fail installation")
        self.assertEqual(self.binary.read_bytes(), b"previous binary")
        self.assertEqual(self.binary.stat().st_mode & 0o777, 0o751)
        self.assertEqual(list(self.binary.parent.glob(".herdr-crossroads-*")), [])
        self.assertFalse((self.base / "escape").exists())
        return result

    def test_installs_fixed_path_executable_without_path_tools(self):
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.binary.read_bytes(), ELF, "validated binary must replace previous runtime file")
        self.assertEqual(self.binary.stat().st_mode & 0o777, 0o755)
        self.assertEqual(result.stdout, f"Installed {ARCHIVE} at target/release/herdr-crossroads\n")
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            self.assertEqual((self.binary.parent / name).read_bytes(), (self.repo / name).read_bytes())
            self.assertEqual((self.binary.parent / name).stat().st_mode & 0o777, 0o644)

    def check_published_html_requires_exact_version_and_equivalent_trusted_markdown(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        # Reproduce the exact published source notice without storing a second
        # multi-megabyte license fixture or reaching the network in tests.
        legacy = generator["generate_html"]().replace("\r\n", "\n").encode()
        self.assertEqual(hashlib.sha256(legacy).hexdigest(),
                         "08b1ee55eea459afb4e0fe55c68ce91c2f1f815df9d1b9505ffecfdc9226b174")
        checksum = f"{hashlib.sha256(ELF).hexdigest()}  herdr-crossroads\n".encode()
        members = [("herdr-crossroads", ELF, tarfile.REGTYPE),
                   ("herdr-crossroads.sha256", checksum, tarfile.REGTYPE),
                   ("LICENSE", (self.repo / "LICENSE").read_bytes(), tarfile.REGTYPE),
                   ("THIRD-PARTY-NOTICES.html", legacy, tarfile.REGTYPE)]
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            path = self.repo / name
            path.write_text(path.read_text().replace('version = "0.0.1"', 'version = "0.1.0"'))
        archive = ARCHIVE.replace("0.0.1", "0.1.0")
        with patch.dict(globals(), ARCHIVE=archive):
            self.package(members=members)
            from test_distribution import prepare_fixture
            fixture = self.base / "distribution-reference"
            fixture.mkdir()
            _, _, payloads = prepare_fixture(fixture, self.assets, public="0.1.0")
            self.assertEqual(payloads, {"0.1.0": ELF})

            def reject_reference():
                with tempfile.TemporaryDirectory(dir=self.base, prefix="bad-reference-") as temporary:
                    with patch.dict(prepare_fixture.__globals__, ROOT=self.repo), self.assertRaises(AssertionError):
                        prepare_fixture(Path(temporary), self.assets, public="0.1.0")

            result = self.install()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(self.binary.read_bytes(), ELF)
            self.assertEqual((self.binary.parent / "THIRD-PARTY-NOTICES.md").read_bytes(),
                             (self.repo / "THIRD-PARTY-NOTICES.md").read_bytes())
            self.assertFalse((self.binary.parent / "THIRD-PARTY-NOTICES.html").exists())
            self.binary.write_bytes(b"previous binary")
            self.binary.chmod(0o751)
            for kind in [tarfile.REGTYPE, tarfile.SYMTYPE, tarfile.LNKTYPE]:
                with self.subTest(kind=kind):
                    self.package(members=members[:-1] + [
                        ("THIRD-PARTY-NOTICES.html", legacy + b"tampered", kind)])
                    self.reject()
                    reject_reference()
            for bad in [members + [members[-1]], members[:-1],
                        members + [("THIRD-PARTY-NOTICES.md", b"extra", tarfile.REGTYPE)],
                        members[:2] + [("LICENSE", b"changed", tarfile.REGTYPE)] + members[3:]]:
                self.package(members=bad)
                self.reject()
                reject_reference()
            self.package(members=members)
            notice = self.repo / "THIRD-PARTY-NOTICES.md"
            notice.write_bytes(notice.read_bytes() + b"changed")
            self.reject()
            reject_reference()
            shutil.copy2(ROOT / notice.name, notice)
            # New Markdown builds use the unchanged version too.
            self.package()
            result = self.install()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.binary.write_bytes(b"previous binary")
            self.binary.chmod(0o751)
        # Exact historic bytes are not an alternate format for other versions.
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            path = self.repo / name
            path.write_text(path.read_text().replace('version = "0.1.0"', 'version = "0.0.1"'))
        self.package(members=members)
        self.reject()

    def test_missing_tampered_or_nonregular_notices_preserve_binary(self):
        checksum = f"{hashlib.sha256(ELF).hexdigest()}  herdr-crossroads\n".encode()
        good = [("herdr-crossroads", ELF, tarfile.REGTYPE),
                ("herdr-crossroads.sha256", checksum, tarfile.REGTYPE)]
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            other = "LICENSE" if name != "LICENSE" else "THIRD-PARTY-NOTICES.md"
            base = good + [(other, (ROOT / other).read_bytes() if (ROOT / other).exists()
                            else b"missing notices", tarfile.REGTYPE)]
            for additions in [[], [(name, b"tampered", tarfile.REGTYPE)],
                              [(name, b"\xff\x00<broken", tarfile.REGTYPE)],
                              [(name, b"", tarfile.SYMTYPE)],
                              [(name, b"", tarfile.LNKTYPE)],
                              [("../" + name, b"bad", tarfile.REGTYPE)]]:
                with self.subTest(name=name, additions=additions):
                    self.package(members=base + additions)
                    self.reject()

    def test_notice_destination_symlinks_rejected_before_binary_replacement(self):
        victim = self.base / "victim"
        victim.write_bytes(b"keep")
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            path = self.binary.parent / name
            path.symlink_to(victim)
            self.reject()
            self.assertEqual(victim.read_bytes(), b"keep")
            path.unlink()

    def test_fresh_destination_created_and_manifest_hook_matches(self):
        self.binary.unlink()
        self.binary.parent.rmdir()
        self.binary.parent.parent.rmdir()
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.binary.read_bytes(), ELF)
        import tomllib
        manifest = tomllib.loads((ROOT / "herdr-plugin.toml").read_text())
        self.assertEqual(manifest["build"], [{"command": ["python3", "scripts/install-binary.py"]}])
        self.assertEqual(manifest["panes"][0]["command"], ["./target/release/herdr-crossroads"])

    def test_relative_asset_directory_rejected(self):
        self.env["CROSSROADS_ASSET_DIR"] = "assets"
        self.reject()

    def test_agreed_version_selects_only_that_version(self):
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            path = self.repo / name
            path.write_text(path.read_text().replace('version = "0.0.1"', 'version = "2.3.4"'))
        self.reject()
        name = ARCHIVE.replace("0.0.1", "2.3.4")
        data = (self.assets / ARCHIVE).read_bytes()
        (self.assets / name).write_bytes(data)
        (self.assets / (name + ".sha256")).write_text(f"{hashlib.sha256(data).hexdigest()}  {name}\n")
        self.assertEqual(self.install().returncode, 0)
        self.assertEqual(self.binary.read_bytes(), ELF)

    def test_missing_exact_version_does_not_select_older_asset(self):
        (self.assets / ARCHIVE).rename(self.assets / ARCHIVE.replace("0.0.1", "0.0.0"))
        self.reject()

    def test_explicit_empty_directory_and_each_asset_are_required(self):
        self.env["CROSSROADS_ASSET_DIR"] = ""
        self.assertIn("CROSSROADS_ASSET_DIR", self.reject().stderr)
        self.env["CROSSROADS_ASSET_DIR"] = str(self.assets)
        for name in [ARCHIVE, ARCHIVE + ".sha256"]:
            with self.subTest(name=name):
                self.package()
                (self.assets / name).unlink()
                self.reject()

    def public_install(self, failure=None, response_url=None, fail_checksum=False):
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        requests = []

        def open_url(url, timeout):
            requests.append((url, timeout))
            if failure and (not fail_checksum or url.endswith(".sha256")):
                raise failure
            name = url.rsplit("/", 1)[1]
            response = io.BytesIO((self.assets / name).read_bytes())
            response.geturl = lambda: response_url or url
            return response

        with patch.dict(os.environ, {"PATH": ""}, clear=True), patch(
                "urllib.request.OpenerDirector.open", side_effect=open_url):
            module["install"]()
        return requests

    def test_public_default_downloads_only_pinned_archive_and_checksum(self):
        requests = self.public_install()
        base = "https://github.com/eisenhauer-dev/herdr-crossroads/releases/download/v0.0.1/"
        self.assertEqual([url for url, _ in requests], [base + ARCHIVE, base + ARCHIVE + ".sha256"])
        self.assertTrue(all(0 < timeout <= 30 for _, timeout in requests))
        self.assertEqual(self.binary.read_bytes(), ELF)
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            self.assertEqual((self.binary.parent / name).read_bytes(), (self.repo / name).read_bytes())

    def test_public_default_uses_agreed_version_not_latest_or_older_asset(self):
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            path = self.repo / name
            path.write_text(path.read_text().replace('version = "0.0.1"', 'version = "2.3.4"'))
        name = ARCHIVE.replace("0.0.1", "2.3.4")
        data = (self.assets / ARCHIVE).read_bytes()
        (self.assets / name).write_bytes(data)
        (self.assets / (name + ".sha256")).write_text(f"{hashlib.sha256(data).hexdigest()}  {name}\n")
        requests = self.public_install(response_url="https://release-assets.githubusercontent.com/asset?signature=fixture")
        self.assertEqual([url for url, _ in requests], [
            "https://github.com/eisenhauer-dev/herdr-crossroads/releases/download/v2.3.4/" + name,
            "https://github.com/eisenhauer-dev/herdr-crossroads/releases/download/v2.3.4/" + name + ".sha256"])
        self.assertEqual(self.binary.read_bytes(), ELF)

    def test_public_network_failures_and_missing_assets_preserve_previous_binary(self):
        for error in [urllib.error.HTTPError("https://github.com/asset", 404, "missing", {}, None),
                      urllib.error.URLError("offline"), TimeoutError("timed out")]:
            if isinstance(error, urllib.error.HTTPError):
                self.addCleanup(error.close)
            with self.subTest(error=error), self.assertRaises((OSError, ValueError)):
                self.public_install(failure=error)
            self.assertEqual(self.binary.read_bytes(), b"previous binary")
        missing_checksum = urllib.error.HTTPError("https://github.com/checksum", 404, "missing checksum", {}, None)
        self.addCleanup(missing_checksum.close)
        with self.assertRaisesRegex(OSError, "missing checksum"):
            self.public_install(failure=missing_checksum, fail_checksum=True)
        self.assertEqual(self.binary.read_bytes(), b"previous binary")

    def test_public_unsupported_host_and_invalid_local_input_never_download(self):
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        for asset in [None, "", "relative"]:
            env = {} if asset is None else {"CROSSROADS_ASSET_DIR": asset}
            with patch.dict(os.environ, env, clear=True), patch(
                    "urllib.request.OpenerDirector.open") as network:
                if asset is None:
                    with patch("platform.machine", return_value="aarch64"), self.assertRaisesRegex(ValueError, "hosts"):
                        module["install"]()
                else:
                    with self.assertRaisesRegex(ValueError, "CROSSROADS_ASSET_DIR"):
                        module["install"]()
                network.assert_not_called()
        self.assertEqual(self.binary.read_bytes(), b"previous binary")

    def test_explicit_local_directory_never_uses_public_download(self):
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        with patch.dict(os.environ, self.env, clear=True), patch(
                "urllib.request.OpenerDirector.open") as network:
            module["install"]()
            network.assert_not_called()
        self.assertEqual(self.binary.read_bytes(), ELF)

    def test_public_invalid_checksum_preserves_previous_binary(self):
        (self.assets / (ARCHIVE + ".sha256")).write_bytes(b"bad hash\n")
        with self.assertRaisesRegex(ValueError, "malformed checksum"):
            self.public_install()
        self.assertEqual(self.binary.read_bytes(), b"previous binary")

    def test_public_download_size_limits_and_redirect_boundaries(self):
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        for url in ["http://github.com/asset", "https://user:password@github.com/asset",
                    "file:///asset"]:
            with self.subTest(url=url), self.assertRaisesRegex(ValueError, "HTTPS"):
                self.public_install(response_url=url)
            self.assertEqual(self.binary.read_bytes(), b"previous binary")
        response = io.BytesIO(b"too large")
        response.geturl = lambda: "https://release-assets.githubusercontent.com/asset?signature=fixture"
        with patch("urllib.request.OpenerDirector.open", return_value=response):
            with self.assertRaisesRegex(ValueError, "size limit"):
                module["download"]("https://github.com/asset", 4)
        handler = module["HTTPSRedirectHandler"]()
        import urllib.request
        request = urllib.request.Request("https://github.com/asset")
        for url in ["http://example.invalid/asset", "https://user@example.invalid/asset"]:
            with self.subTest(redirect=url), self.assertRaisesRegex(ValueError, "HTTPS"):
                handler.redirect_request(request, None, 302, "Found", {}, url)
        redirected = handler.redirect_request(request, None, 302, "Found", {},
                                             "https://release-assets.githubusercontent.com/asset?signature=fixture")
        self.assertTrue(redirected.full_url.startswith("https://release-assets.githubusercontent.com/"))

    def test_oversized_expanded_archive_preserves_previous_binary(self):
        import gzip
        (self.assets / ARCHIVE).write_bytes(gzip.compress(bytes(128 * 1024 * 1024 + 1)))
        self.archive_checksum()
        result = self.reject()
        self.assertIn("expanded archive exceeds size limit", result.stderr)

    def test_truncated_downloaded_gzip_is_handled_without_traceback(self):
        archive = self.assets / ARCHIVE
        archive.write_bytes(archive.read_bytes()[:-5])
        self.archive_checksum()
        result = self.reject()
        self.assertNotIn("Traceback", result.stderr)

    def test_archive_hash_and_canonical_checksum_required(self):
        for text in ["0" * 64 + f"  {ARCHIVE}\n", "bad\n",
                     "A" * 64 + f"  {ARCHIVE}\n",
                     "0" * 64 + "  ../escape\n", "0" * 64 + f" *{ARCHIVE}\n"]:
            with self.subTest(text=text):
                (self.assets / (ARCHIVE + ".sha256")).write_text(text)
                self.reject()
        self.package()
        (self.assets / ARCHIVE).write_bytes(b"corrupted")
        self.reject()

    def test_binary_hash_and_checksum_syntax_required(self):
        for text in [b"bad", ("0" * 64 + "  herdr-crossroads\n").encode(),
                     (hashlib.sha256(ELF).hexdigest() + "  ../herdr-crossroads\n").encode()]:
            with self.subTest(text=text):
                self.package(internal=text)
                self.reject()

    def test_exact_regular_members_only(self):
        checksum = f"{hashlib.sha256(ELF).hexdigest()}  herdr-crossroads\n".encode()
        good = [("herdr-crossroads", ELF, tarfile.REGTYPE),
                ("herdr-crossroads.sha256", checksum, tarfile.REGTYPE),
                ("LICENSE", (ROOT / "LICENSE").read_bytes(), tarfile.REGTYPE),
                ("THIRD-PARTY-NOTICES.md", (ROOT / "THIRD-PARTY-NOTICES.md").read_bytes(), tarfile.REGTYPE)]
        cases = [good[:1], good + [good[0]], good + [("extra", b"", tarfile.REGTYPE)],
                 [("../escape", ELF, tarfile.REGTYPE), *good[1:]],
                 [("/escape", ELF, tarfile.REGTYPE), *good[1:]]]
        cases.extend([[("herdr-crossroads", b"", kind), *good[1:]]
                      for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.DIRTYPE, tarfile.FIFOTYPE]])
        for members in cases:
            with self.subTest(members=[(name, kind) for name, _, kind in members]):
                self.package(members=members)
                self.reject()

    def test_invalid_tar_and_wrong_elf_rejected(self):
        (self.assets / ARCHIVE).write_bytes(b"not a tar")
        self.archive_checksum()
        self.reject()
        for data in [b"not ELF", ELF[:20], ELF.replace(b"\x7fELF", b"oops"),
                     ELF[:4] + b"\x01" + ELF[5:], ELF[:18] + b"\xb7\x00" + ELF[20:]]:
            with self.subTest(data=data):
                self.package(data=data)
                self.reject()

    def test_special_contiguous_member_is_not_a_regular_release_member(self):
        checksum = f"{hashlib.sha256(ELF).hexdigest()}  herdr-crossroads\n".encode()
        self.package(members=[("herdr-crossroads", ELF, tarfile.CONTTYPE),
                              ("herdr-crossroads.sha256", checksum, tarfile.REGTYPE),
                              ("LICENSE", (ROOT / "LICENSE").read_bytes(), tarfile.REGTYPE),
                              ("THIRD-PARTY-NOTICES.md", (ROOT / "THIRD-PARTY-NOTICES.md").read_bytes(), tarfile.REGTYPE)])
        self.reject()

    def test_unsupported_hosts_fail_before_installation(self):
        for patch in ["platform.system=lambda: 'Darwin'; ", "platform.machine=lambda: 'aarch64'; ",
                      "platform.libc_ver=lambda: ('musl', '1.2'); "]:
            with self.subTest(patch=patch):
                self.reject(patch)

    def test_version_mismatch_preserves_binary(self):
        path = self.repo / "herdr-plugin.toml"
        path.write_text(path.read_text().replace('version = "0.0.1"', 'version = "0.2.0"'))
        self.reject()

    def test_destination_and_ancestor_symlinks_rejected(self):
        outside = self.base / "outside"
        outside.mkdir()
        victim = outside / "herdr-crossroads"
        victim.write_bytes(b"outside")
        self.binary.unlink()
        self.binary.symlink_to(victim)
        self.assertNotEqual(self.install().returncode, 0)
        self.assertEqual(victim.read_bytes(), b"outside")
        self.binary.unlink()
        self.binary.parent.rmdir()
        self.binary.parent.symlink_to(outside, target_is_directory=True)
        self.assertNotEqual(self.install().returncode, 0)
        self.assertEqual(victim.read_bytes(), b"outside")

    def test_failed_atomic_replace_preserves_previous_and_cleans_staging(self):
        result = self.reject("import os; os.replace=lambda *a, **k: (_ for _ in ()).throw(OSError('forced replace failure')); ")
        self.assertEqual(result.stderr, "Binary installation failed: forced replace failure\n")
        self.assertNotIn("Traceback", result.stderr)

    def test_failed_staging_sync_preserves_previous(self):
        result = self.reject("import os; os.fsync=lambda *a: (_ for _ in ()).throw(OSError('forced staging failure')); ")
        self.assertEqual(result.stderr, "Binary installation failed: forced staging failure\n")
        self.assertNotIn("Traceback", result.stderr)

    def test_failed_binary_replace_keeps_notices_and_previous_binary(self):
        patch = ("import os; replace=os.replace; "
                 "os.replace=lambda src, dst, **kw: "
                 "(_ for _ in ()).throw(OSError('forced binary failure')) "
                 "if dst == 'herdr-crossroads' else replace(src, dst, **kw); ")
        self.reject(patch)
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            self.assertEqual((self.binary.parent / name).read_bytes(), (self.repo / name).read_bytes())


if __name__ == "__main__":
    unittest.main()
