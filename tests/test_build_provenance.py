"""Collector contracts using fake subprocess results, never a second real link."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("build_provenance", ROOT / "scripts/build-provenance.py")
provenance = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provenance)


class BuildProvenanceTests(unittest.TestCase):
    def test_subprocesses_use_stable_locale_for_dpkg_diversion_diagnostics(self):
        with patch.object(provenance.subprocess, "run") as command:
            provenance.run(["dpkg-query", "-S", "/lib64/ld-linux-x86-64.so.2"])
        self.assertEqual(command.call_args.kwargs.get("env", {}).get("LC_ALL"), "C")

    def test_normalization_replaces_only_own_path_prefixes(self):
        root = self.directory / "repo"
        text = f'{root}/file {root}-other/file /usr/lib/Scrt1.o'
        self.assertEqual(provenance.normalize(text, root),
                         f'<repo>/file {root}-other/file /usr/lib/Scrt1.o')

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="link-evidence-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.binary = self.directory / "herdr-crossroads"
        self.stem = self.directory / "herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu"
        self.startup = self.directory / "Scrt1.o"
        self.startup.write_bytes(b"startup fixture")
        self.archive = self.directory / "libgcc.a"
        self.archive.write_bytes(b"archive fixture")
        self.calls = []

    def fake_run(self, argv, *, check=True):
        self.calls.append(argv)
        stdout = ""
        stderr = ""
        if argv[:2] == ["cargo", "rustc"]:
            self.binary.write_bytes(b"exact linked binary")
            self.binary.chmod(0o755)
            Path(str(self.stem) + ".link.map").write_text(
                f"{ROOT}/src/main.rs {Path.home()}/.cargo/registry /usr/lib/Scrt1.o\n")
            stdout = f'cc "{ROOT}/target/main.o" -fuse-ld=lld\n'
            stderr = (f"warning: linker stdout: LLD 23.1.1\n"
                      f"         {self.startup}\n         {self.archive}(runtime.o)\n")
        elif argv[0] == "gcc" and argv[1].startswith("-print-file-name="):
            stdout = str(self.startup) + "\n"
        elif argv[:2] == ["dpkg-query", "-S"]:
            stdout = f"libc6-dev:amd64: {argv[2]}\n"
        elif argv[:2] == ["dpkg-query", "-W"]:
            stdout = "libc6-dev:amd64\t2.39-0ubuntu8.6\tglibc\t2.39-0ubuntu8.6\n"
        else:
            stdout = "tool version fixture\n"
        return subprocess.CompletedProcess(argv, 0, stdout, stderr)

    def test_same_link_records_argv_versions_packages_hashes_and_raw_evidence(self):
        with patch.object(provenance, "run", side_effect=self.fake_run), patch.dict(
            os.environ, {"RUSTFLAGS": f"-L {ROOT}/target", "PRIVATE_TEST_SECRET": "must-not-record"}
        ):
            provenance.collect("x86_64-unknown-linux-gnu", self.binary, self.stem)
        text = Path(str(self.stem) + ".provenance.json").read_text()
        data = json.loads(text)
        self.assertEqual(sum(call[:2] == ["cargo", "rustc"] for call in self.calls), 1)
        self.assertEqual(self.calls[0], ["cargo", "clean", "--release", "--target",
                                         "x86_64-unknown-linux-gnu", "--package", "herdr-crossroads"])
        self.assertIn("--trace", " ".join(data["build_argv"]))
        self.assertIn("--print", data["build_argv"])
        self.assertEqual(data["binary"]["sha256"], hashlib.sha256(self.binary.read_bytes()).hexdigest())
        self.assertEqual(data["map"]["sha256"], provenance.sha256(Path(str(self.stem) + ".link.map")))
        self.assertIn("LLD 23.1.1", data["link_diagnostics"])
        self.assertEqual(len(data["trace_files"]), 2)
        self.assertEqual(data["trace_files"][0]["packages"][0]["source"], "glibc")
        self.assertEqual(data["gcc_resolved_candidates"][0]["sha256"], provenance.sha256(self.startup))
        self.assertIn("not proof", data["limits"])
        self.assertNotIn(str(ROOT), text)
        self.assertNotIn(str(Path.home()), text)
        self.assertNotIn("must-not-record", text)
        self.assertIn("<repo>", text)
        self.assertIn("<cargo>", Path(str(self.stem) + ".link.map").read_text())
        self.assertIn("/usr/lib/Scrt1.o", Path(str(self.stem) + ".link.map").read_text())

    def test_trace_is_not_a_universal_map_parser(self):
        self.assertEqual(provenance.traced_paths(
            '/usr/lib/libgcc.a(member.o)\n /usr/lib/Scrt1.o\n  123 456 /usr/lib/not-a-trace.o\n'
            'cc "/private/argv.o"\n'), ["/usr/lib/Scrt1.o", "/usr/lib/libgcc.a"])

    def test_missing_inputs_are_explicit_not_fabricated_hashes(self):
        with patch.object(provenance, "run") as command:
            record = provenance.file_record(self.directory / "expired-temporary.o")
        self.assertTrue(record["unavailable"])
        self.assertNotIn("sha256", record)
        command.assert_not_called()

    def test_unowned_file_keeps_hash_without_inventing_package(self):
        with patch.object(provenance, "run", return_value=subprocess.CompletedProcess([], 1, "", "no owner")):
            record = provenance.file_record(self.startup)
        self.assertEqual(record["packages"], [])
        self.assertEqual(record["sha256"], provenance.sha256(self.startup))

    def test_failure_does_not_write_success_evidence(self):
        with patch.object(provenance, "run", return_value=subprocess.CompletedProcess([], 19, "", "link failed")):
            with self.assertRaises(subprocess.CalledProcessError):
                provenance.collect("x86_64-unknown-linux-gnu", self.binary, self.stem)
        self.assertFalse(Path(str(self.stem) + ".provenance.json").exists())

    def test_no_map_or_nonexecutable_output_is_not_provenance(self):
        for missing in ["map", "binary", "executable"]:
            with self.subTest(missing=missing):
                def incomplete(argv, **kwargs):
                    result = self.fake_run(argv, **kwargs)
                    if argv[1] == "clean":
                        return result
                    if missing == "executable":
                        self.binary.chmod(0o644)
                    else:
                        (Path(str(self.stem) + ".link.map") if missing == "map" else self.binary).unlink()
                    return result
                with patch.object(provenance, "run", side_effect=incomplete):
                    with self.assertRaisesRegex(RuntimeError, "nonempty map"):
                        provenance.collect("x86_64-unknown-linux-gnu", self.binary, self.stem)
                self.assertFalse(Path(str(self.stem) + ".provenance.json").exists())


if __name__ == "__main__":
    unittest.main()
