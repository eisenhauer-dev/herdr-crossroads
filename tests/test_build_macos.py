"""Packaging contracts with mocked tools; never native execution evidence."""
import hashlib
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def helper():
    path = ROOT / "scripts/build-macos.py"
    if not path.exists():
        return None
    loader = importlib.machinery.SourceFileLoader("build_macos", str(path))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


class BuildMacosTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir="/tmp/opencode")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.module = helper()
        self.calls = []
        environment = patch.dict(os.environ, {}, clear=True)
        environment.start()
        self.addCleanup(environment.stop)
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            (self.root / name).write_text(name)

    def command(self, argv):
        self.calls.append(argv)
        output = ""
        if argv[:2] == ["uname", "-s"]:
            output = "Darwin"
        elif argv[:2] == ["uname", "-m"]:
            output = self.arch
        elif "check-version.py" in argv[1]:
            output = "0.2.0"
        elif "build-provenance.py" in argv[1]:
            binary, stem = map(Path, argv[-2:])
            binary.parent.mkdir(parents=True, exist_ok=True)
            binary.write_bytes(b"exact native fixture")
            binary.chmod(0o755)
            Path(str(stem) + ".link.map").write_text("map")
            Path(str(stem) + ".provenance.json").write_text(json.dumps({
                "binary": {"sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}}))
        return subprocess.CompletedProcess(argv, 0, output, "")

    def build(self, arch="arm64", effect=None):
        self.assertIsNotNone(self.module, "native macOS build/gate/package entry point required")
        self.arch = arch
        with patch.object(self.module, "ROOT", self.root), patch.object(
                self.module, "run", side_effect=effect or self.command):
            return self.module.build()

    def test_both_architectures_package_exact_binary_and_four_files(self):
        for arch, target in [("arm64", "aarch64-apple-darwin"), ("x86_64", "x86_64-apple-darwin")]:
            with self.subTest(arch=arch):
                self.calls.clear()
                archive = self.build(arch)
                self.assertEqual(archive.name, f"herdr-crossroads-0.2.0-{target}.tar.gz")
                gates = [c for c in self.calls if c[1].startswith("tests/pty_")]
                self.assertEqual([c[1] for c in gates], [f"tests/pty_{g}.py" for g in
                                                     ["latency", "render", "keymaps", "hints"]])
                self.assertTrue(all(len(c) == 3 for c in gates))
                self.assertEqual(sum("build-provenance.py" in c[1] for c in self.calls), 1)
                with tarfile.open(archive) as package:
                    self.assertEqual(package.getnames(), ["herdr-crossroads", "herdr-crossroads.sha256",
                                                          "LICENSE", "THIRD-PARTY-NOTICES.md"])
                    for member in package:
                        self.assertTrue(member.isfile())
                        self.assertEqual((member.uid, member.gid, member.uname, member.gname), (0, 0, "", ""))
                        self.assertEqual(member.mode, 0o755 if member.name == "herdr-crossroads" else 0o644)
                    binary = package.extractfile("herdr-crossroads").read()
                    self.assertEqual(binary, b"exact native fixture")
                    self.assertEqual(package.extractfile("herdr-crossroads.sha256").read().decode(),
                                     hashlib.sha256(binary).hexdigest() + "  herdr-crossroads\n")
                self.assertEqual(Path(str(archive) + ".sha256").read_text(),
                                 hashlib.sha256(archive.read_bytes()).hexdigest() + f"  {archive.name}\n")

    def test_each_failed_gate_removes_current_artifacts(self):
        for gate in ["latency", "render", "keymaps", "hints"]:
            self.build()
            output = self.root / "target/builds"
            (output / "unrelated").write_text("keep")
            def fail(argv):
                if argv[1] == f"tests/pty_{gate}.py":
                    raise subprocess.CalledProcessError(23, argv)
                return self.command(argv)
            with self.assertRaises(subprocess.CalledProcessError):
                self.build(effect=fail)
            self.assertEqual([p.name for p in output.iterdir()], ["unrelated"])

    def test_changed_binary_after_gate_is_never_packaged(self):
        def mutate(argv):
            result = self.command(argv)
            if argv[1] == "tests/pty_hints.py":
                Path(argv[2]).write_bytes(b"mutated")
            return result
        with self.assertRaisesRegex(RuntimeError, "changed"):
            self.build(effect=mutate)
        self.assertEqual(list((self.root / "target/builds").iterdir()), [])

    def test_unsupported_architecture_fails_before_build(self):
        with self.assertRaisesRegex(RuntimeError, "native"):
            self.build("i386")
        self.assertFalse(any("build-provenance.py" in c[1] for c in self.calls))

    def test_build_notice_and_packaging_errors_remove_stale_outputs(self):
        for stage in ["third-party-notices.py", "build-provenance.py", "archive"]:
            with self.subTest(stage=stage):
                self.build()
                def fail(argv):
                    if stage in argv[1]:
                        raise subprocess.CalledProcessError(19, argv)
                    return self.command(argv)
                with patch.object(self.module.tarfile, "open", side_effect=OSError("archive failure")) if stage == "archive" else patch.object(self.module, "digest", wraps=self.module.digest):
                    with self.assertRaises((OSError, subprocess.CalledProcessError)):
                        self.build(effect=fail)
                self.assertEqual(list((self.root / "target/builds").iterdir()), [])

    def test_compiler_override_is_rejected_not_silently_used(self):
        self.build()
        self.calls.clear()
        with patch.dict(os.environ, {"RUSTFLAGS": "-C link-arg=/unreviewed.a"}):
            with self.assertRaisesRegex(RuntimeError, "RUSTFLAGS"):
                self.build()
        self.assertFalse(any("build-provenance.py" in c[1] for c in self.calls))
        self.assertEqual(list((self.root / "target/builds").iterdir()), [])


if __name__ == "__main__":
    unittest.main()
