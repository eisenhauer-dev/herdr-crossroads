"""Installer tests: synthetic ELF/Mach-O, mocked HTTP and fake curl; no network/Herdr/Rust.

The full historical notice fixture is exercised separately by test_notices.py
on builders with the cached sources and Rust documentation installed.
"""

import hashlib
import io
import json
import os
from pathlib import Path
import platform
import runpy
import shutil
import ssl
import struct
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
NATIVE_SYSTEM = platform.system()
NATIVE_MACHINE = platform.machine()
LINUX_HOST = ("platform.system=lambda: 'Linux'; platform.machine=lambda: 'x86_64'; "
              "platform.libc_ver=lambda: ('glibc', '2.39'); ")


def macho(cpu=0x0100000c, filetype=2, target_platform=1, minimum=0x000f0000):
    # Thin little-endian Mach-O64 with one LC_BUILD_VERSION (macOS 15.0).
    return (struct.pack("<8I", 0xfeedfacf, cpu, 0, filetype, 1, 24, 0, 0)
            + struct.pack("<6I", 0x32, 24, target_platform, minimum, minimum, 0))


def darwin_host(machine="arm64", version="15.0"):
    return (f"platform.system=lambda: 'Darwin'; platform.machine=lambda: {machine!r}; "
            f"platform.mac_ver=lambda: ({version!r}, ('', '', ''), {machine!r}); ")


class InstallBinaryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-install-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name).resolve()
        for name, value in [("system", "Linux"), ("machine", "x86_64"),
                            ("libc_ver", ("glibc", "2.39"))]:
            host = patch.object(platform, name, return_value=value)
            host.start()
            self.addCleanup(host.stop)
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
        code = ("import platform, runpy; " + LINUX_HOST + patch +
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

    def test_manifest_supports_linux_and_macos(self):
        manifest = tomllib.loads((ROOT / "herdr-plugin.toml").read_text())
        self.assertEqual(manifest["platforms"], ["linux", "macos", "windows"])

    def test_darwin_process_architecture_selects_thin_matching_asset(self):
        for machine, target, cpu in [("arm64", "aarch64", 0x0100000c),
                                     ("aarch64", "aarch64", 0x0100000c),
                                     ("x86_64", "x86_64", 0x01000007)]:
            with self.subTest(machine=machine), patch.dict(
                    globals(), ARCHIVE=f"herdr-crossroads-0.0.1-{target}-apple-darwin.tar.gz"):
                self.package(data=macho(cpu))
                result = self.install(darwin_host(machine))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.binary.read_bytes(), macho(cpu))
                self.assertIn(ARCHIVE, result.stdout)
                self.assertEqual(self.binary.stat().st_mode & 0o777, 0o755)
                for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
                    self.assertEqual((self.binary.parent / name).stat().st_mode & 0o777, 0o644)

    def test_darwin_old_unknown_and_unsupported_hosts_preserve_installation(self):
        for machine in ["arm64", "x86_64"]:
            for version in ["14.9.9", "10.15.7", "", "unknown", "15", "15.x", "15.0\n"]:
                with self.subTest(machine=machine, version=version):
                    self.assertIn("macOS 15.0", self.reject(darwin_host(machine, version)).stderr)
        self.reject(darwin_host("i386"))

    def test_darwin_wrong_binary_headers_and_platform_preserve_installation(self):
        with patch.dict(globals(), ARCHIVE="herdr-crossroads-0.0.1-aarch64-apple-darwin.tar.gz"):
            good = macho()
            for data in [ELF, b"not Mach-O", good[:31], macho(0x01000007),
                         macho(filetype=1), macho(filetype=6), macho(target_platform=2),
                         macho(minimum=0x00100000), good[:32],
                         b"\xca\xfe\xba\xbe" + good[4:],
                         b"\xcf\xfa\xed\xfe"[::-1] + good[4:],
                         good[:20] + struct.pack("<I", 1000) + good[24:],
                         good[:36] + struct.pack("<I", 0) + good[40:]]:
                with self.subTest(data=data):
                    self.package(data=data)
                    self.assertIn("Mach-O", self.reject(darwin_host()).stderr)

    def test_darwin_intel_rejects_arm_and_all_universal_magic(self):
        with patch.dict(globals(), ARCHIVE="herdr-crossroads-0.0.1-x86_64-apple-darwin.tar.gz"):
            for data in [macho(), *(magic + macho(0x01000007)[4:] for magic in
                                   [b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca",
                                    b"\xca\xfe\xba\xbf", b"\xbf\xba\xfe\xca"])]:
                self.package(data=data)
                self.assertIn("Mach-O", self.reject(darwin_host("x86_64")).stderr)

    def test_darwin_public_downloads_process_target_and_old_os_never_downloads(self):
        for machine, target, cpu in [("arm64", "aarch64", 0x0100000c),
                                     ("x86_64", "x86_64", 0x01000007)]:
            with self.subTest(machine=machine), patch.dict(
                    globals(), ARCHIVE=f"herdr-crossroads-0.0.1-{target}-apple-darwin.tar.gz"):
                self.package(data=macho(cpu))
                with patch("platform.system", return_value="Darwin"), patch(
                        "platform.machine", return_value=machine), patch(
                        "platform.mac_ver", return_value=("15.0", (), machine)):
                    requests = self.public_install()
                    self.assertTrue(requests[0][0].endswith(ARCHIVE))
                    self.assertTrue(requests[1][0].endswith(ARCHIVE + ".sha256"))
                    self.assertEqual(self.binary.read_bytes(), macho(cpu))
                    module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
                    with patch("platform.mac_ver", return_value=("14.9", (), machine)), patch(
                            "subprocess.Popen") as curl, patch(
                            "urllib.request.OpenerDirector.open") as network:
                        with self.assertRaisesRegex(ValueError, "macOS 15.0"):
                            module["install"]()
                        curl.assert_not_called()
                        network.assert_not_called()

    def test_darwin_newer_os_accepts_compatible_deployment_version(self):
        with patch.dict(globals(), ARCHIVE="herdr-crossroads-0.0.1-aarch64-apple-darwin.tar.gz"):
            self.package(data=macho())
            result = self.install(darwin_host(version="26.0.1"))
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_linux_rejects_macho_and_darwin_missing_asset_has_no_fallback(self):
        self.package(data=macho())
        self.reject()
        self.package()
        self.reject(darwin_host())

    def test_native_host_installation_path(self):
        # Exercise unmocked OS detection and native descriptor operations. The
        # header-only payload is not an executable smoke test (distribution owns it).
        if NATIVE_SYSTEM == "Darwin":
            target = "aarch64" if NATIVE_MACHINE in ["arm64", "aarch64"] else "x86_64"
            data = macho(0x0100000c if target == "aarch64" else 0x01000007)
            name = f"herdr-crossroads-0.0.1-{target}-apple-darwin.tar.gz"
        else:
            data, name = ELF, ARCHIVE
        with patch.dict(globals(), ARCHIVE=name):
            self.package(data=data)
            result = subprocess.run([sys.executable, str(self.repo / "scripts/install-binary.py")],
                                    env=self.env, cwd=self.base, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.binary.read_bytes(), data)

    def check_published_html_requires_exact_version_and_equivalent_trusted_markdown(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        # Reproduce the exact published source notice without storing a second
        # multi-megabyte license fixture or reaching the network in tests.
        legacy = generator["generate_html"]().replace("\r\n", "\n").encode()
        self.assertEqual(hashlib.sha256(legacy).hexdigest(),
                          "08b1ee55eea459afb4e0fe55c68ce91c2f1f815df9d1b9505ffecfdc9226b174")
        # The historical checkout must retain its exact Markdown inventory too;
        # the current Windows-expanded inventory is deliberately not equivalent.
        historical_markdown = generator["generate"](windows=False).encode()
        self.assertEqual(hashlib.sha256(historical_markdown).hexdigest(),
                         "ba243d492753e7d6ce0832a14827d635c9066b354ed715456a492c46bde43e56")
        (self.repo / "THIRD-PARTY-NOTICES.md").write_bytes(historical_markdown)
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
            with patch.dict(prepare_fixture.__globals__, ROOT=self.repo):
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

    def check_published_markdown_with_windows_expanded_checkout(self):
        generator = runpy.run_path(str(ROOT / "scripts/third-party-notices.py"))
        # Freeze the v0.2.0 inventory by digest, independently of today's default
        # generator output; reproduce its verbatim sources without a duplicate fixture.
        released = generator["generate"](windows=False).encode()
        self.assertEqual(hashlib.sha256(released).hexdigest(),
                         "ba243d492753e7d6ce0832a14827d635c9066b354ed715456a492c46bde43e56")
        expanded = (self.repo / "THIRD-PARTY-NOTICES.md").read_bytes()
        self.assertNotEqual(released, expanded)
        checksum = f"{hashlib.sha256(ELF).hexdigest()}  herdr-crossroads\n".encode()
        members = [("herdr-crossroads", ELF, tarfile.REGTYPE),
                   ("herdr-crossroads.sha256", checksum, tarfile.REGTYPE),
                   ("LICENSE", (self.repo / "LICENSE").read_bytes(), tarfile.REGTYPE),
                   ("THIRD-PARTY-NOTICES.md", released, tarfile.REGTYPE)]
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            path = self.repo / name
            path.write_text(path.read_text().replace('version = "0.0.1"', 'version = "0.2.0"'))
        hosts = [("x86_64-unknown-linux-gnu", ELF, ""),
                 ("aarch64-apple-darwin", macho(), darwin_host()),
                 ("x86_64-apple-darwin", macho(0x01000007), darwin_host("x86_64"))]
        for target, binary, host in hosts:
            with self.subTest(target=target), patch.dict(
                    globals(), ARCHIVE=f"herdr-crossroads-0.2.0-{target}.tar.gz"):
                payload = [("herdr-crossroads", binary, tarfile.REGTYPE),
                           ("herdr-crossroads.sha256",
                            f"{hashlib.sha256(binary).hexdigest()}  herdr-crossroads\n".encode(),
                            tarfile.REGTYPE), *members[2:]]
                self.package(members=payload)
                result = self.install(host)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.binary.read_bytes(), binary)
                self.assertEqual((self.binary.parent / "THIRD-PARTY-NOTICES.md").read_bytes(), released)
                self.binary.write_bytes(b"previous binary")
                self.binary.chmod(0o751)
                for notice in [released + b"changed", expanded + b"changed"]:
                    self.package(members=payload[:-1] + [(members[-1][0], notice, tarfile.REGTYPE)])
                    self.assertIn("trusted checkout", self.reject(host).stderr)
                self.package(members=payload)
                (self.repo / "THIRD-PARTY-NOTICES.md").write_bytes(expanded + b"changed")
                self.assertIn("trusted checkout", self.reject(host).stderr)
                (self.repo / "THIRD-PARTY-NOTICES.md").write_bytes(expanded)
                self.package(members=payload)
                (self.assets / (ARCHIVE + ".sha256")).write_bytes(b"0" * 64 + f"  {ARCHIVE}\n".encode())
                self.assertIn("SHA256 mismatch", self.reject(host).stderr)
        # No unpublished Windows asset may use the old Unix-only inventory.
        with patch.dict(globals(), ARCHIVE="herdr-crossroads-0.2.0-x86_64-pc-windows-msvc.tar.gz"):
            self.package(members=[(name.replace("herdr-crossroads", "herdr-crossroads.exe"), data, kind)
                                  for name, data, kind in members])
            host = ("platform.system=lambda: 'Windows'; platform.machine=lambda: 'AMD64'; "
                    "platform.win32_ver=lambda: ('10', '10.0.19045', '', ''); ")
            self.assertIn("trusted checkout", self.reject(host).stderr)
        # The pinned inventory is not an alternate notice for other versions.
        previous = "0.2.0"
        for version in ["0.0.1", "0.1.0", "0.3.0"]:
            for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
                path = self.repo / name
                path.write_text(path.read_text().replace(f'version = "{previous}"', f'version = "{version}"'))
            with patch.dict(globals(), ARCHIVE=f"herdr-crossroads-{version}-x86_64-unknown-linux-gnu.tar.gz"):
                self.package(members=members)
                self.assertIn("trusted checkout", self.reject().stderr)
            previous = version

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
        self.assertEqual(manifest["build"], [
            {"platforms": ["linux", "macos"], "command": ["python3", "scripts/install-binary.py"]},
            {"platforms": ["windows"], "command": ["python", "scripts/install-binary.py"]},
        ])
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

    def fake_curl(self, body=None):
        tools = self.base / "tools"
        tools.mkdir(exist_ok=True)
        curl = tools / "curl"
        curl.write_text(
            f"#!{sys.executable}\n"
            "import json, pathlib, sys, time\n"
            f"with open({str(self.base / 'curl-args')!r}, 'a') as log:\n"
            "    log.write(json.dumps(sys.argv[1:]) + '\\n')\n"
            f"assets = pathlib.Path({str(self.assets)!r})\n"
            + (body if body is not None else
               "sys.stdout.buffer.write((assets / sys.argv[-1].rsplit('/', 1)[1]).read_bytes())\n"))
        curl.chmod(0o755)
        return tools

    def curl_environment(self, tools):
        return {"PATH": str(tools), "https_proxy": "http://user:password@proxy.invalid",
                "ALL_PROXY": "socks5://proxy.invalid", "HOME": str(self.base)}

    def test_curl_security_arguments_and_pinned_urls(self):
        tools = self.fake_curl()
        (self.base / ".curlrc").write_text("insecure\nproxy = http://proxy.invalid\n")
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        with patch.dict(os.environ, self.curl_environment(tools), clear=True), patch(
                "urllib.request.OpenerDirector.open") as python_download:
            module["install"]()
            python_download.assert_not_called()
        calls = [json.loads(line) for line in (self.base / "curl-args").read_text().splitlines()]
        base = "https://github.com/eisenhauer-dev/herdr-crossroads/releases/download/v0.0.1/"
        self.assertEqual(calls, [[
            "--disable", "--proto", "=https", "--proto-redir", "=https",
            "--disallow-username-in-url", "--location", "--fail", "--silent", "--show-error",
            "--noproxy", "*", "--max-time", "30", "--url", base + name,
        ] for name in [ARCHIVE, ARCHIVE + ".sha256"]])

    def test_curl_failures_never_fall_back_and_preserve_previous_binary(self):
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        # Protocol downgrade, credentialed redirect, HTTP, timeout, TLS and
        # unsupported-option statuses: none permits switching TLS clients.
        for status in [1, 3, 22, 28, 60, 2]:
            with self.subTest(status=status):
                tools = self.fake_curl(f"sys.stdout.buffer.write(b'partial')\nsys.exit({status})\n")
                with patch.dict(os.environ, self.curl_environment(tools), clear=True), patch(
                        "urllib.request.OpenerDirector.open") as python_download:
                    with self.assertRaisesRegex(ValueError, "curl.*failed"):
                        module["install"]()
                    python_download.assert_not_called()
                self.assertEqual(self.binary.read_bytes(), b"previous binary")
                self.assertEqual(self.binary.stat().st_mode & 0o777, 0o751)
                self.assertEqual(list(self.binary.parent.glob(".herdr-crossroads-*")), [])

    def test_curl_bounded_unknown_length_output_kills_and_reaps_child(self):
        tools = self.fake_curl("sys.stdout.buffer.write(b'x' * 65536)\n"
                               "sys.stdout.buffer.flush()\ntime.sleep(60)\n")
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        children = []
        popen = subprocess.Popen

        def start(*args, **kwargs):
            child = popen(*args, **kwargs)
            children.append(child)
            return child

        with patch.dict(os.environ, self.curl_environment(tools), clear=True), patch(
                "subprocess.Popen", side_effect=start), patch(
                "urllib.request.OpenerDirector.open", side_effect=AssertionError("must use curl")):
            with self.assertRaisesRegex(ValueError, "size limit"):
                module["download"]("https://github.com/asset", 1024)
        self.assertEqual(len(children), 1)
        self.assertIsNotNone(children[0].returncode, "oversize child must be reaped")
        self.assertTrue(children[0].stdout.closed)

    def test_curl_read_error_kills_and_reaps_child_without_fallback(self):
        tools = self.fake_curl("time.sleep(60)\n")
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        children = []
        popen = subprocess.Popen

        def start(*args, **kwargs):
            child = popen(*args, **kwargs)
            children.append(child)
            read = patch.object(child.stdout, "read", side_effect=OSError("forced pipe failure"))
            read.start()
            self.addCleanup(read.stop)
            return child

        with patch.dict(os.environ, self.curl_environment(tools), clear=True), patch(
                "subprocess.Popen", side_effect=start), patch(
                "urllib.request.OpenerDirector.open") as python_download:
            with self.assertRaisesRegex(OSError, "forced pipe failure"):
                module["install"]()
            python_download.assert_not_called()
        self.assertEqual(len(children), 1)
        self.assertIsNotNone(children[0].returncode)
        self.assertTrue(children[0].stdout.closed)
        self.assertEqual(self.binary.read_bytes(), b"previous binary")

    def test_curl_launch_error_never_falls_back(self):
        tools = self.fake_curl()
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        with patch.dict(os.environ, self.curl_environment(tools), clear=True), patch(
                "subprocess.Popen", side_effect=OSError("cannot execute curl")), patch(
                "urllib.request.OpenerDirector.open") as python_download:
            with self.assertRaisesRegex(OSError, "cannot execute curl"):
                module["install"]()
            python_download.assert_not_called()
        self.assertEqual(self.binary.read_bytes(), b"previous binary")

    def test_curl_checksum_failure_and_size_limits_preserve_previous_binary(self):
        tools = self.fake_curl()
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        original = module["download"]
        limits = []

        def download(url, limit):
            limits.append(limit)
            return original(url, limit)

        checksum = self.assets / (ARCHIVE + ".sha256")
        for data, message in [(b"bad hash\n", "malformed checksum"),
                              (b"0" * 64 + f"  {ARCHIVE}\n".encode(), "SHA256 mismatch"),
                              (b"x" * 1025, "size limit")]:
            with self.subTest(message=message):
                limits.clear()
                checksum.write_bytes(data)
                with patch.dict(os.environ, self.curl_environment(tools), clear=True), patch.dict(
                        module["install"].__globals__, download=download):
                    with self.assertRaisesRegex(ValueError, message):
                        module["install"]()
                self.assertEqual(limits, [64 * 1024 * 1024, 1024])
                self.assertEqual(self.binary.read_bytes(), b"previous binary")
        self.fake_curl("if sys.argv[-1].endswith('.sha256'):\n    sys.exit(22)\n"
                       "sys.stdout.buffer.write((assets / sys.argv[-1].rsplit('/', 1)[1]).read_bytes())\n")
        with patch.dict(os.environ, self.curl_environment(tools), clear=True), patch(
                "urllib.request.OpenerDirector.open") as python_download:
            with self.assertRaisesRegex(ValueError, "curl.*failed"):
                module["install"]()
            python_download.assert_not_called()
        self.assertEqual(self.binary.read_bytes(), b"previous binary")

    def test_curl_exact_limit_and_initial_url_boundary(self):
        tools = self.fake_curl("sys.stdout.buffer.write(b'abcd')\n")
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        with patch.dict(os.environ, self.curl_environment(tools), clear=True), patch(
                "urllib.request.OpenerDirector.open", side_effect=AssertionError("must use curl")):
            self.assertEqual(module["download"]("https://github.com/asset", 4), b"abcd")
            with patch("subprocess.Popen") as child:
                for url in ["http://github.com/asset", "https://user:password@github.com/asset",
                            "https://user@github.com/asset", "file:///asset"]:
                    with self.subTest(url=url), self.assertRaisesRegex(ValueError, "HTTPS"):
                        module["download"](url, 4)
                child.assert_not_called()

    def test_available_curl_installs_despite_python_strict_tls_failure(self):
        tools = self.fake_curl()
        module = runpy.run_path(str(self.repo / "scripts/install-binary.py"))
        failure = urllib.error.URLError(ssl.SSLCertVerificationError(
            "CERTIFICATE_VERIFY_FAILED: Missing Authority Key Identifier"))
        with patch.dict(os.environ, {"PATH": str(tools)}, clear=True), patch(
                "urllib.request.OpenerDirector.open", side_effect=failure) as python_download:
            try:
                module["install"]()
            except OSError as error:
                self.fail(f"available curl must avoid Python's strict TLS failure: {error}")
            python_download.assert_not_called()
        self.assertEqual(self.binary.read_bytes(), ELF)
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            self.assertEqual((self.binary.parent / name).read_bytes(), (self.repo / name).read_bytes())

    def test_absent_curl_keeps_python_strict_tls_failure(self):
        failure = urllib.error.URLError(ssl.SSLCertVerificationError(
            "CERTIFICATE_VERIFY_FAILED: Missing Authority Key Identifier"))
        with self.assertRaisesRegex(urllib.error.URLError, "Missing Authority Key Identifier"):
            self.public_install(failure=failure)
        self.assertEqual(self.binary.read_bytes(), b"previous binary")

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
                "urllib.request.OpenerDirector.open") as network, patch(
                "shutil.which") as discovery, patch("subprocess.Popen") as curl:
            module["install"]()
            network.assert_not_called()
            discovery.assert_not_called()
            curl.assert_not_called()
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
        with patch.dict(os.environ, {"PATH": ""}, clear=True), patch(
                "urllib.request.OpenerDirector.open", return_value=response):
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
