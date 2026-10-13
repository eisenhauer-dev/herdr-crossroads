"""Local installer tests: synthetic ELF bytes, no Herdr or Rust invocation."""

import hashlib
import io
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
ARCHIVE = "herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
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
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            shutil.copy2(ROOT / name, self.repo / name)
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
            path.write_text(path.read_text().replace('version = "0.1.0"', 'version = "2.3.4"'))
        self.reject()
        name = ARCHIVE.replace("0.1.0", "2.3.4")
        data = (self.assets / ARCHIVE).read_bytes()
        (self.assets / name).write_bytes(data)
        (self.assets / (name + ".sha256")).write_text(f"{hashlib.sha256(data).hexdigest()}  {name}\n")
        self.assertEqual(self.install().returncode, 0)
        self.assertEqual(self.binary.read_bytes(), ELF)

    def test_missing_exact_version_does_not_select_older_asset(self):
        (self.assets / ARCHIVE).rename(self.assets / ARCHIVE.replace("0.1.0", "0.0.9"))
        self.reject()

    def test_environment_and_each_asset_are_required(self):
        del self.env["CROSSROADS_ASSET_DIR"]
        self.assertIn("CROSSROADS_ASSET_DIR", self.reject().stderr)
        self.env["CROSSROADS_ASSET_DIR"] = str(self.assets)
        for name in [ARCHIVE, ARCHIVE + ".sha256"]:
            with self.subTest(name=name):
                self.package()
                (self.assets / name).unlink()
                self.reject()

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
                ("herdr-crossroads.sha256", checksum, tarfile.REGTYPE)]
        cases = [good[:1], good + [good[0]], good + [("extra", b"", tarfile.REGTYPE)],
                 [("../escape", ELF, tarfile.REGTYPE), good[1]],
                 [("/escape", ELF, tarfile.REGTYPE), good[1]]]
        cases.extend([[("herdr-crossroads", b"", kind), good[1]]
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
                              ("herdr-crossroads.sha256", checksum, tarfile.REGTYPE)])
        self.reject()

    def test_unsupported_hosts_fail_before_installation(self):
        for patch in ["platform.system=lambda: 'Darwin'; ", "platform.machine=lambda: 'aarch64'; ",
                      "platform.libc_ver=lambda: ('musl', '1.2'); "]:
            with self.subTest(patch=patch):
                self.reject(patch)

    def test_version_mismatch_preserves_binary(self):
        path = self.repo / "herdr-plugin.toml"
        path.write_text(path.read_text().replace('version = "0.1.0"', 'version = "0.2.0"'))
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
        self.assertEqual(result.stderr, "Local binary installation failed: forced replace failure\n")
        self.assertNotIn("Traceback", result.stderr)

    def test_failed_staging_sync_preserves_previous(self):
        result = self.reject("import os; os.fsync=lambda *a: (_ for _ in ()).throw(OSError('forced staging failure')); ")
        self.assertEqual(result.stderr, "Local binary installation failed: forced staging failure\n")
        self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
