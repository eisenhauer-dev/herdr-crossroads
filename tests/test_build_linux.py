"""Exercise fixed-target packaging with fake build/PTY tools and real tar/SHA256."""

import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"
ARCHIVE = f"herdr-crossroads-0.1.0-{TARGET}.tar.gz"
GATES = ["latency", "render", "keymaps", "hints"]


class BuildLinuxTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(
            dir="/tmp/opencode", prefix="crossroads-build-"
        )
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name) / "repo"
        (self.repo / "scripts").mkdir(parents=True)
        source = ROOT / "scripts/build-linux.sh"
        if source.exists():
            shutil.copy2(source, self.repo / "scripts/build-linux.sh")
        shutil.copy2(ROOT / "scripts/build-provenance.py", self.repo / "scripts/build-provenance.py")
        checker = ROOT / "scripts/check-version.py"
        if checker.exists():
            shutil.copy2(checker, self.repo / "scripts/check-version.py")
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml", "LICENSE", "THIRD-PARTY-NOTICES.html"]:
            if not (ROOT / name).exists():
                continue
            shutil.copy2(ROOT / name, self.repo / name)
        if (ROOT / "scripts/third-party-notices.py").exists():
            shutil.copy2(ROOT / "scripts/third-party-notices.py", self.repo / "scripts/third-party-notices.py")
        shutil.copytree(ROOT / "LICENSES", self.repo / "LICENSES")
        self.bin = Path(self.temporary.name) / "bin"
        self.bin.mkdir()
        self.log = Path(self.temporary.name) / "calls"
        self.env = {
            "PATH": f"{self.bin}:/usr/bin:/bin", "LC_ALL": "C",
            "CALL_LOG": str(self.log),
            # A caller's Cargo output override must not select an old binary.
            "CARGO_TARGET_DIR": str(self.repo / "other-target"),
        }
        self.tool("uname", '#!/bin/sh\ncase "$1" in -s) echo "${HOST_OS:-Linux}";; -m) echo "${HOST_ARCH:-x86_64}";; esac\n')
        self.tool("cargo", '''#!/bin/sh
test "$1" != --version || { echo 'cargo fixture'; exit 0; }
test "$1" != clean || exit "${CLEAN_STATUS:-0}"
printf 'cargo|%s\n' "$*" >> "$CALL_LOG"
test "${BUILD_STATUS:-0}" = 0 || exit "$BUILD_STATUS"
test "${OMIT_BINARY:-0}" = 0 || exit 0
mkdir -p "$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release"
printf 'fresh binary\n' > "$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release/herdr-crossroads"
chmod "${BINARY_MODE:-755}" "$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release/herdr-crossroads"
for arg in "$@"; do
    case "$arg" in link-arg=-Wl,-Map=*)
        test "${OMIT_MAP:-0}" = 0 || continue
        map=${arg#link-arg=-Wl,-Map=}
        printf 'map for %s /usr/lib/Scrt1.o\n' "$PWD" > "$map"
    ;; esac
done
printf 'cc -Wl,-Map=fixture\nLLD fixture\n'
''')
        self.tool("gcc", '#!/bin/sh\ncase "$1" in --version) echo "gcc fixture";; *) exec /usr/bin/gcc "$@";; esac\n')
        self.tool("rustc", '#!/bin/sh\nprintf "%s\\n" "rustc 1.99.0 (b940084d7 2026-09-28)"\n')
        self.tool("python3", f'''#!/bin/sh
case "$1" in scripts/check-version.py|scripts/third-party-notices.py|scripts/build-provenance.py) exec "{sys.executable}" "$@";; esac
printf 'pty|%s\n' "$*" >> "$CALL_LOG"
test "$(cat "$2")" = 'fresh binary' || exit 98
test "$1" != "tests/pty_${{FAIL_GATE:-none}}.py" || exit 23
''')
        self.tool("tar", '#!/bin/sh\nprintf "package\\n" >> "$CALL_LOG"\nexec /usr/bin/tar "$@"\n')
        self.output = self.repo / "target/builds"
        self.output.mkdir(parents=True)
        self.binary = self.repo / f"target/{TARGET}/release/herdr-crossroads"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_text("stale binary\n")
        self.binary.chmod(0o755)
        for name in [ARCHIVE, ARCHIVE + ".sha256", ARCHIVE.removesuffix('.tar.gz') + '.provenance.json', ARCHIVE.removesuffix('.tar.gz') + '.link.map']:
            (self.output / name).write_text("stale artifact\n")
        (self.output / "unrelated.txt").write_text("keep\n")
        (self.output / f"herdr-crossroads-0.0.9-{TARGET}.tar.gz").write_text("prior version\n")

    def tool(self, name, text):
        path = self.bin / name
        path.write_text(text)
        path.chmod(0o755)

    def build(self):
        # No helper means no build/gates/package: assert the missing domain result,
        # rather than treating a missing executable/setup error as TDD RED.
        helper = self.repo / "scripts/build-linux.sh"
        command = ["sh", str(helper)] if helper.exists() else ["sh", "-c", ":"]
        return subprocess.run(command, cwd=self.bin, env=self.env,
                              text=True, capture_output=True)

    def calls(self):
        return self.log.read_text().splitlines() if self.log.exists() else []

    def expected_calls(self):
        stem = self.output / ARCHIVE.removesuffix('.tar.gz')
        return [f"cargo|rustc --release --locked --bin herdr-crossroads --target {TARGET} -- --print link-args -C link-arg=-Wl,-Map={stem}.link.map -C link-arg=-Wl,--trace -C link-arg=-Wl,-v", *[
            f"pty|tests/pty_{gate}.py {self.binary}" for gate in GATES
        ]]

    def assert_no_artifact(self):
        self.assertFalse((self.output / ARCHIVE).exists(), "failed build must remove stale archive")
        self.assertFalse((self.output / (ARCHIVE + ".sha256")).exists())
        for suffix in ['.provenance.json', '.link.map']:
            self.assertFalse((self.output / (ARCHIVE.removesuffix('.tar.gz') + suffix)).exists())
        self.assertEqual(list(self.output.glob("*.tmp")), [])
        self.assertEqual((self.output / "unrelated.txt").read_text(), "keep\n")

    def test_success_packages_exact_tested_binary_and_portable_checksums(self):
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), [*self.expected_calls(), "package"],
                         "all full PTYs must precede packaging")
        archive = self.output / ARCHIVE
        self.assertEqual(archive.read_bytes()[:2], b"\x1f\x8b", "successful build must replace the versioned archive")
        with tarfile.open(archive) as package:
            self.assertEqual(package.getnames(), ["herdr-crossroads", "herdr-crossroads.sha256", "LICENSE", "THIRD-PARTY-NOTICES.html"])
            for name in ["LICENSE", "THIRD-PARTY-NOTICES.html"]:
                self.assertEqual(package.extractfile(name).read(), (ROOT / name).read_bytes())
            binary = package.getmember("herdr-crossroads")
            self.assertEqual(binary.mode & 0o111, 0o111)
            self.assertEqual((binary.uid, binary.gid, binary.uname, binary.gname),
                             (0, 0, "", ""))
            data = package.extractfile(binary).read()
            self.assertEqual(data, b"fresh binary\n")
            checksum = package.extractfile("herdr-crossroads.sha256").read().decode()
            self.assertEqual(checksum, f"{hashlib.sha256(data).hexdigest()}  herdr-crossroads\n")
        self.assertEqual((self.output / (ARCHIVE + ".sha256")).read_text(),
                         f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {ARCHIVE}\n")
        self.assertIn(ARCHIVE, result.stdout)
        self.assertNotIn(str(self.repo), (self.output / (ARCHIVE + ".sha256")).read_text())

    def test_exact_build_has_fresh_binary_bound_provenance_outside_archive(self):
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        stem = ARCHIVE.removesuffix('.tar.gz')
        evidence = self.output / f"{stem}.provenance.json"
        self.assertTrue(evidence.exists(), "successful packaging requires current-build provenance")
        data = json.loads(evidence.read_text())
        self.assertEqual(data["binary"]["sha256"], hashlib.sha256(self.binary.read_bytes()).hexdigest())
        self.assertEqual(data["map"]["sha256"], hashlib.sha256((self.output / f"{stem}.link.map").read_bytes()).hexdigest())
        self.assertNotIn(str(self.repo), evidence.read_text())

    def test_version_mismatch_blocks_build_and_all_gates(self):
        plugin = self.repo / "herdr-plugin.toml"
        plugin.write_text(plugin.read_text().replace('version = "0.1.0"', 'version = "0.2.0"'))
        result = self.build()
        self.assertNotEqual(result.returncode, 0, "version mismatch must block packaging")
        self.assertEqual(self.calls(), [], "version mismatch must fail before Cargo")
        self.assertIn("version", result.stderr.lower())

    def test_stale_or_missing_notices_block_packaging(self):
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.html"]:
            with self.subTest(name=name):
                path = self.repo / name
                if path.exists():
                    path.unlink()
                self.assertNotEqual(self.build().returncode, 0)
                self.assert_no_artifact()
                if (ROOT / name).exists():
                    shutil.copy2(ROOT / name, path)
        lock = self.repo / "Cargo.lock"
        lock.write_text(lock.read_text().replace('version = "0.34.0"', 'version = "0.34.1"'))
        self.assertNotEqual(self.build().returncode, 0)
        self.assert_no_artifact()

    def test_manifest_and_toolchain_changes_require_notice_review(self):
        manifest = self.repo / "Cargo.toml"
        original = manifest.read_text()
        manifest.write_text(original + '\n[features]\nnotice-review = []\n')
        self.assertNotEqual(self.build().returncode, 0)
        self.assert_no_artifact()
        manifest.write_text(original)
        self.tool("rustc", '#!/bin/sh\necho "rustc 1.99.1 (different toolchain)"\n')
        self.assertNotEqual(self.build().returncode, 0)
        self.assert_no_artifact()

    def test_prior_versions_are_not_reused_or_removed(self):
        self.assertEqual(self.build().returncode, 0)
        self.assertEqual((self.output / f"herdr-crossroads-0.0.9-{TARGET}.tar.gz").read_text(), "prior version\n")

    def test_agreed_version_determines_artifact_name_without_workflow_side_effects(self):
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            path = self.repo / name
            text = path.read_text()
            if name == "Cargo.lock":
                text = text.replace('name = "herdr-crossroads"\nversion = "0.1.0"',
                                    'name = "herdr-crossroads"\nversion = "2.3.4"')
            else:
                text = text.replace('version = "0.1.0"', 'version = "2.3.4"')
            path.write_text(text)
        metadata = self.repo / "workflow-output"
        metadata.write_text("keep\n")
        self.env["GITHUB_OUTPUT"] = str(metadata)
        self.assertEqual(self.build().returncode, 0)
        archive = self.output / f"herdr-crossroads-2.3.4-{TARGET}.tar.gz"
        self.assertEqual(archive.read_bytes()[:2], b"\x1f\x8b")
        self.assertEqual((self.output / ARCHIVE).read_text(), "stale artifact\n")
        self.assertEqual(metadata.read_text(), "keep\n")

    def test_invalid_or_missing_version_data_blocks_before_build(self):
        path = self.repo / "herdr-plugin.toml"
        for contents in [None, "[broken", 'version = "../escape"\n']:
            with self.subTest(contents=contents):
                if contents is None:
                    path.unlink()
                else:
                    path.write_text(contents)
                self.assertNotEqual(self.build().returncode, 0)
                self.assertEqual(self.calls(), [])
                self.assertEqual(self.binary.read_text(), "stale binary\n")
                path.write_text('version = "0.1.0"\n')

    def test_workflow_step_outputs_exact_name_only_after_success(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        step = workflow.split("        id: package\n        run: |\n", 1)[1].split("      - name: Upload", 1)[0]
        script = "\n".join(line[10:] for line in step.splitlines())
        metadata = self.repo / "workflow-output"
        self.env["GITHUB_OUTPUT"] = str(metadata)
        for failure in ["build", "version", None]:
            with self.subTest(failure=failure):
                metadata.write_text("")
                self.env["BUILD_STATUS"] = "19" if failure == "build" else "0"
                (self.repo / "herdr-plugin.toml").write_text(
                    'version = "0.2.0"\n' if failure == "version" else 'version = "0.1.0"\n')
                result = subprocess.run(["sh", "-e", "-c", script], cwd=self.repo,
                                        env=self.env, text=True, capture_output=True)
                if failure:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(metadata.read_text(), "")
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(metadata.read_text(), f"artifact={ARCHIVE.removesuffix('.tar.gz')}\n")
                    self.assertEqual((self.output / ARCHIVE).read_bytes()[:2], b"\x1f\x8b")

    def test_failed_build_removes_stale_artifacts_and_skips_ptys(self):
        self.env["BUILD_STATUS"] = "19"
        result = self.build()
        self.assertEqual(result.returncode, 19)
        self.assertEqual(self.calls(), self.expected_calls()[:1])
        self.assert_no_artifact()
        self.assertNotIn(ARCHIVE, result.stdout)

    def test_package_clean_failure_cannot_replay_cached_provenance(self):
        self.env["CLEAN_STATUS"] = "29"
        self.assertEqual(self.build().returncode, 29)
        self.assertEqual(self.calls(), [])
        self.assert_no_artifact()

    def test_provenance_collection_failure_blocks_ptys_and_packaging(self):
        self.tool("gcc", '#!/bin/sh\ntest "$1" != --version || exit 31\nexec /usr/bin/gcc "$@"\n')
        self.assertEqual(self.build().returncode, 31)
        self.assertEqual(self.calls(), self.expected_calls()[:1])
        self.assert_no_artifact()

    def test_unreviewed_system_builder_stops_before_cargo_and_ptys(self):
        self.tool("dpkg-query", '#!/bin/sh\ncase "$1" in -W) printf "unknown glibc unknown\\n";; *) exec /usr/bin/dpkg-query "$@";; esac\n')
        result = self.build()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("reviewed system builder", result.stderr)
        self.assertEqual(self.calls(), [])
        self.assert_no_artifact()

    def test_success_without_new_binary_cannot_test_stale_binary(self):
        self.env["OMIT_BINARY"] = "1"
        self.assertNotEqual(self.build().returncode, 0)
        self.assertEqual(self.calls(), self.expected_calls()[:1])
        self.assert_no_artifact()

    def test_success_without_new_map_cannot_reuse_stale_provenance(self):
        self.env["OMIT_MAP"] = "1"
        self.assertNotEqual(self.build().returncode, 0)
        self.assertEqual(self.calls(), self.expected_calls()[:1])
        self.assert_no_artifact()

    def test_nonexecutable_binary_stops_before_ptys(self):
        self.env["BINARY_MODE"] = "644"
        self.assertNotEqual(self.build().returncode, 0)
        self.assertEqual(self.calls(), self.expected_calls()[:1])
        self.assert_no_artifact()

    def test_each_failed_pty_stops_packaging_and_later_gates(self):
        for index, gate in enumerate(GATES):
            with self.subTest(gate=gate):
                self.env["FAIL_GATE"] = gate
                if self.log.exists():
                    self.log.unlink()
                self.assertEqual(self.build().returncode, 23)
                self.assertEqual(self.calls(), self.expected_calls()[:index + 2])
                self.assert_no_artifact()

    def test_packaging_or_checksum_failure_leaves_no_valid_output(self):
        for name in ["tar", "sha256sum"]:
            with self.subTest(tool=name):
                self.tool(name, "#!/bin/sh\n" + (
                    'printf partial > "$5"\n' if name == "tar" else "printf partial\n"
                ) + "exit 31\n")
                self.assertEqual(self.build().returncode, 31)
                self.assert_no_artifact()
                (self.bin / name).unlink()

    def test_archive_checksum_failure_removes_already_created_archive(self):
        self.tool("sha256sum", f'''#!/bin/sh
test "$1" != '{ARCHIVE}' || exit 37
exec /usr/bin/sha256sum "$@"
''')
        self.assertEqual(self.build().returncode, 37)
        self.assertEqual(self.calls(), [*self.expected_calls(), "package"])
        self.assert_no_artifact()

    def test_missing_cargo_does_not_test_stale_binary(self):
        (self.bin / "cargo").unlink()
        self.assertNotEqual(self.build().returncode, 0)
        self.assertEqual(self.calls(), [])
        self.assert_no_artifact()

    def test_unsupported_native_hosts_rejected_before_build(self):
        for key, value in [("HOST_OS", "Darwin"), ("HOST_ARCH", "aarch64")]:
            with self.subTest(host=value):
                self.env[key] = value
                self.assertNotEqual(self.build().returncode, 0)
                self.assertEqual(self.calls(), [])
                self.assert_no_artifact()
                del self.env[key]


if __name__ == "__main__":
    unittest.main()
