"""Portable Windows host/PE contract tests; native installation is a separate gate."""

from pathlib import Path
import hashlib
import io
import os
import runpy
import shutil
import struct
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
INSTALL = runpy.run_path(str(ROOT / "scripts/install-binary.py"))
TARGET = "x86_64-pc-windows-msvc"


def pe():
    data = bytearray(512)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3c, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    # AMD64, one section, PE32+ optional header, executable (not a DLL).
    struct.pack_into("<HHIIIHH", data, 0x84, 0x8664, 1, 0, 0, 0, 240, 0x22)
    struct.pack_into("<H", data, 0x98, 0x20b)
    struct.pack_into("<H", data, 0x98 + 68, 3)  # console subsystem
    return bytes(data)


class WindowsDistributionTests(unittest.TestCase):
    def test_windows_archive_checks_precede_native_installation(self):
        with tempfile.TemporaryDirectory(prefix="crossroads-windows-archive-") as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            for name in ("Cargo.toml", "Cargo.lock", "herdr-plugin.toml", "LICENSE", "THIRD-PARTY-NOTICES.md"):
                shutil.copy2(ROOT / name, root / name)
            shutil.copy2(ROOT / "scripts/check-version.py", root / "scripts/check-version.py")
            version = runpy.run_path(str(root / "scripts/check-version.py"))["release_version"]()
            archive = root / f"herdr-crossroads-{version}-{TARGET}.tar.gz"
            checksum = f"{hashlib.sha256(pe()).hexdigest()}  herdr-crossroads.exe\n".encode()
            good = {"herdr-crossroads.exe": pe(), "herdr-crossroads.exe.sha256": checksum,
                    **{name: (root / name).read_bytes() for name in ("LICENSE", "THIRD-PARTY-NOTICES.md")}}
            calls = []
            original = runpy.run_path
            def load(path):
                if Path(path).name == "windows-files.py":
                    return {"install_files": lambda destination, files: calls.append((destination, files))}
                return original(path)
            install = INSTALL["install"]
            for changed, succeeds in [({}, True), ({"herdr-crossroads.exe": b"not PE"}, False),
                                       ({"herdr-crossroads.exe.sha256": b"bad hash"}, False),
                                       ({"LICENSE": b"tampered"}, False),
                                       ({"THIRD-PARTY-NOTICES.md": b"tampered"}, False),
                                       ({"../escape": b"extra"}, False)]:
                members = {**good, **changed}
                if changed.get("herdr-crossroads.exe"):
                    members["herdr-crossroads.exe.sha256"] = (
                        hashlib.sha256(members["herdr-crossroads.exe"]).hexdigest() + "  herdr-crossroads.exe\n").encode()
                with tarfile.open(archive, "w:gz") as package:
                    for name, data in members.items():
                        member = tarfile.TarInfo(name)
                        member.size = len(data)
                        package.addfile(member, io.BytesIO(data))
                Path(str(archive) + ".sha256").write_bytes(
                    f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n".encode())
                calls.clear()
                with self.subTest(changed=list(changed)), patch.dict(
                        install.__globals__, __file__=str(root / "scripts/install-binary.py"),
                        host_target=lambda: TARGET), patch.dict(os.environ, CROSSROADS_ASSET_DIR=str(root)), patch(
                        "runpy.run_path", side_effect=load):
                    if succeeds:
                        install()
                        self.assertEqual(calls, [(root, {name: good[name] for name in (
                            "LICENSE", "THIRD-PARTY-NOTICES.md", "herdr-crossroads.exe")})])
                    else:
                        with self.assertRaises(ValueError):
                            install()
                        self.assertEqual(calls, [], "invalid archive must not reach native writes")

    def test_native_windows_x64_target(self):
        for machine in ("AMD64", "x86_64"):
            with patch("platform.system", return_value="Windows"), patch(
                    "platform.machine", return_value=machine), patch(
                    "platform.win32_ver", return_value=("10", "10.0.19045", "", "")):
                self.assertEqual(INSTALL["host_target"](), TARGET)

    def test_windows_rejects_old_unknown_or_non_x64_host(self):
        for machine, version in [("ARM64", "10.0.22631"), ("x86", "10.0.19045"),
                                 ("AMD64", "6.3.9600"), ("AMD64", "")]:
            with self.subTest(machine=machine, version=version), patch(
                    "platform.system", return_value="Windows"), patch(
                    "platform.machine", return_value=machine), patch(
                    "platform.win32_ver", return_value=("", version, "", "")):
                with self.assertRaises(ValueError):
                    INSTALL["host_target"]()

    def test_pe_x64_console_executable(self):
        INSTALL["verify_binary"](pe(), TARGET)

    def test_pe_rejects_wrong_arch_dll_gui_truncated_and_spoofed_headers(self):
        bad = [b"", pe()[:200], b"not PE" + pe()[6:]]
        for offset, fmt, value in [(0x3c, "I", 500), (0x84, "H", 0xaa64),
                                   (0x96, "H", 0x2022), (0x98, "H", 0x10b),
                                   (0x98 + 68, "H", 2), (0x94, "H", 2),
                                   (0x86, "H", 0)]:
            data = bytearray(pe())
            struct.pack_into("<" + fmt, data, offset, value)
            bad.append(bytes(data))
        for data in bad:
            with self.subTest(length=len(data)), self.assertRaises(ValueError):
                INSTALL["verify_binary"](data, TARGET)


@unittest.skipUnless(os.name == "nt", "requires Windows directory/reparse-point semantics")
class WindowsFileInstallationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="crossroads-windows-install-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "checkout with spaces"
        self.root.mkdir()
        self.install = runpy.run_path(str(ROOT / "scripts/windows-files.py"))["install_files"]

    def test_atomic_replace_and_unrelated_config_retention(self):
        self.install(self.root, {"herdr-crossroads.exe": b"first"})
        config = self.root / "config.jsonc"
        config.write_bytes(b"keep me")
        self.install(self.root, {"herdr-crossroads.exe": b"second", "LICENSE": b"notice"})
        self.assertEqual((self.root / "target/release/herdr-crossroads.exe").read_bytes(), b"second")
        self.assertEqual(config.read_bytes(), b"keep me")
        self.assertEqual(sorted(p.name for p in (self.root / "target/release").iterdir()),
                         ["LICENSE", "herdr-crossroads.exe"])

    def test_junction_parent_and_directory_destination_are_rejected(self):
        outside = Path(self.temp.name) / "outside"
        outside.mkdir()
        subprocess.run(["cmd.exe", "/d", "/c", "mklink", "/J", str(self.root / "target"), str(outside)],
                       check=True, capture_output=True)
        try:
            with self.assertRaises((ValueError, OSError)):
                self.install(self.root, {"herdr-crossroads.exe": b"escape"})
            self.assertEqual(list(outside.iterdir()), [])
        finally:
            os.rmdir(self.root / "target")
        (self.root / "target/release/herdr-crossroads.exe").mkdir(parents=True)
        with self.assertRaises(ValueError):
            self.install(self.root, {"herdr-crossroads.exe": b"not a directory"})

    def test_pinned_parent_cannot_be_renamed_during_installation(self):
        module = runpy.run_path(str(ROOT / "scripts/windows-files.py"))
        for relative in ("target/release", "target", "."):
            source = self.root / relative
            moved = source.with_name(source.name + "-moved")
            with self.subTest(relative=relative):
                try:
                    with module["pinned_destination"](self.root) as destination:
                        with self.assertRaises(OSError):
                            source.rename(moved)
                        self.assertTrue(destination.is_dir())
                finally:
                    if moved.exists():
                        moved.rename(source)
                # Closing the pin must release the rename restriction.
                source.rename(moved)
                moved.rename(source)

    def test_parent_swap_is_blocked_before_staging(self):
        outside = Path(self.temp.name) / "outside"
        outside.mkdir()
        release = self.root / "target/release"
        moved = release.with_name("moved")
        open_file = os.open
        attempts = []

        def attempt_swap(path, flags, mode):
            attempts.append(path)
            try:
                release.rename(moved)
            except OSError:
                pass
            else:
                subprocess.run(["cmd.exe", "/d", "/c", "mklink", "/J", str(release), str(outside)],
                               check=True, capture_output=True, timeout=10)
            return open_file(path, flags, mode)

        try:
            with patch("os.open", side_effect=attempt_swap):
                self.install(self.root, {"herdr-crossroads.exe": b"verified"})
            self.assertEqual(len(attempts), 1)
            self.assertFalse(moved.exists(), "a pinned directory must not be renamed")
            self.assertEqual(list(outside.iterdir()), [])
            self.assertEqual((release / "herdr-crossroads.exe").read_bytes(), b"verified")
        finally:
            if moved.exists():
                if release.exists():
                    os.rmdir(release)
                moved.rename(release)

    def test_existing_delete_handle_prevents_installation(self):
        module = runpy.run_path(str(ROOT / "scripts/windows-files.py"))
        kernel = module["KERNEL"]
        # DELETE access with all sharing enabled: the installer must refuse to
        # proceed while another handle already has permission to rename root.
        handle = kernel.CreateFileW(str(self.root), 0x10000, 7, None, 3, 0x02200000, None)
        self.assertNotEqual(handle, module["ctypes"].c_void_p(-1).value)
        try:
            with self.assertRaises(OSError):
                self.install(self.root, {"herdr-crossroads.exe": b"must not write"})
            self.assertFalse((self.root / "target").exists())
        finally:
            kernel.CloseHandle(handle)


if __name__ == "__main__":
    unittest.main()
