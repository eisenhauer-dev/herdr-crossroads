"""Collector contracts using fake subprocess results, never a second real link."""

import errno
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("build_provenance", ROOT / "scripts/build-provenance.py")
provenance = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provenance)


class BuildProvenanceTests(unittest.TestCase):
    def native_run(self, argv, *, check=True):
        self.calls.append(argv)
        outputs = {
            ("uname", "-s"): "Darwin\n", ("uname", "-m"): self.native_arch + "\n",
            ("sysctl", "-in", "sysctl.proc_translated"): "0\n",
            ("rustc", "-vV"): f"release: 1.99.0\nhost: {self.native_target}\ncommit-hash: {'b' * 40}\n",
            ("rustc", "--print", "sysroot"): str(self.directory / "rust"),
            ("xcrun", "--sdk", "macosx", "--show-sdk-path"): str(self.directory),
            ("xcrun", "--sdk", "macosx", "--show-sdk-version"): "15.5\n",
            ("xcrun", "--find", "clang"): "/usr/bin/clang\n",
            ("sw_vers", "-productVersion"): "15.7\n",
            ("lipo", "-archs", str(self.binary)): self.native_arch + "\n",
            ("otool", "-hv", str(self.binary)): "MH_MAGIC_64 ARM64 ALL EXECUTE\n",
            ("otool", "-l", str(self.binary)): "Load command 1\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform 1\n    minos 15.0\n      sdk 15.5\n",
            ("otool", "-L", str(self.binary)): str(self.binary) + ":\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n",
            ("codesign", "--display", "--verbose=4", str(self.binary)): "Signature=adhoc\n",
        }
        if argv[:2] == ["cargo", "rustc"]:
            symbols = ROOT / f"target/{self.native_target}/release/deps" / (
                "rustcYx5yPX" if self.native_arch == "arm64" else "rustcHF47HB") / "symbols.o"
            project_object = ROOT / f"target/{self.native_target}/release/deps/herdr_crossroads-123.rcgu.o"
            self.binary.write_bytes(b"one ad-hoc signed native fixture")
            self.binary.chmod(0o755)
            Path(str(self.stem) + ".link.map").write_text(
                "# Object files:\n[  0] linker synthesized\n[  1] "
                + str(project_object)
                + "\n[  2] " + str(symbols) + "\n# Sections:\n# Symbols:\n")
            return subprocess.CompletedProcess(argv, 0, f'clang "{symbols}"\n',
                                              f"warning: linker stdout: {symbols}\n         {project_object}\n")
        return subprocess.CompletedProcess(argv, 0, outputs.get(tuple(argv), "version fixture\n"), "")

    def native_collect(self, target="aarch64-apple-darwin", effect=None):
        self.native_target = target
        self.native_arch = "arm64" if target.startswith("aarch64") else "x86_64"
        with patch.dict(os.environ, {"MACOSX_DEPLOYMENT_TARGET": "15.0"}), patch.object(
                provenance, "run", side_effect=effect or self.native_run):
            provenance.collect(target, self.binary, self.stem)

    def test_darwin_same_link_inspected_signed_and_hashed_on_both_architectures(self):
        for target in ["aarch64-apple-darwin", "x86_64-apple-darwin"]:
            self.calls.clear()
            self.native_collect(target)
            data = json.loads(Path(str(self.stem) + ".provenance.json").read_text())
            self.assertEqual(data["target"], target)
            self.assertEqual(data["binary"]["sha256"], provenance.sha256(self.binary))
            self.assertEqual(data["map"]["sha256"], provenance.sha256(Path(str(self.stem) + ".link.map")))
            self.assertEqual(sum(c[:2] == ["cargo", "rustc"] for c in self.calls), 1)
            arguments = " ".join(data["build_argv"])
            for required in ["-Wl,-map,", "-Wl,-t", "-Wl,-v", "-Wl,-adhoc_codesign"]:
                self.assertIn(required, arguments)
            for forbidden in ["--trace", "-Map=", "--strip", "codesign --sign"]:
                self.assertNotIn(forbidden, arguments)
            self.assertFalse(any(c[0] in ["gcc", "dpkg-query", "strip"] for c in self.calls))
            self.assertIn("Signature=adhoc", data["inspections"]["signature"])
            symbols = ("rustcYx5yPX" if target.startswith("aarch64") else "rustcHF47HB") + "/symbols.o"
            self.assertTrue(any(entry.endswith(symbols) for entry in data["object_inputs"]))
            self.assertIn(symbols, data["link_diagnostics"])
            self.assertNotIn(str(ROOT), Path(str(self.stem) + ".provenance.json").read_text())

    def test_darwin_generated_symbols_accepts_stdout_trace(self):
        def native(argv, **kwargs):
            result = self.native_run(argv, **kwargs)
            if argv[:2] == ["cargo", "rustc"]:
                result.stdout += result.stderr.splitlines()[0].removeprefix("warning: linker stdout: ") + "\n"
                result.stderr = "Apple ld fixture\n"
            return result

        self.native_collect(effect=native)
        self.assertTrue(Path(str(self.stem) + ".provenance.json").is_file())

    def sdk_layout(self):
        sdk = self.directory.resolve() / "SDKs/MacOSX.sdk"
        (sdk / "usr/lib/system").mkdir(parents=True, exist_ok=True)
        alias = sdk.parent / "MacOSX15.5.sdk"
        if not alias.exists():
            alias.symlink_to(sdk.name, target_is_directory=True)
        stub = sdk / "usr/lib/libiconv.2.tbd"
        stub.write_text("--- !tapi-tbd\ntbd-version:     4\n"
                        "targets:         [ arm64-macos, x86_64-macos ]\n"
                        "install-name:    '/usr/lib/libiconv.2.dylib'\n...\n")
        unversioned = sdk / "usr/lib/libiconv.tbd"
        if not unversioned.exists():
            unversioned.symlink_to(stub.name)
        return sdk, alias, stub, unversioned

    def sdk_native_run(self, sdk, map_entries, trace_entries):
        def native(argv, **kwargs):
            result = self.native_run(argv, **kwargs)
            if argv == ["xcrun", "--sdk", "macosx", "--show-sdk-path"]:
                result.stdout = str(sdk) + "\n"
            elif argv[:2] == ["cargo", "rustc"]:
                map_path = Path(str(self.stem) + ".link.map")
                entries = "".join(f"[ {index + 3}] {entry}\n" for index, entry in enumerate(map_entries))
                map_path.write_text(map_path.read_text().replace("# Sections:\n", entries + "# Sections:\n"))
                result.stderr += "".join(f"         {entry}\n" for entry in trace_entries)
            return result
        return native

    def test_darwin_sdk_stubs_are_separate_hashed_same_link_interfaces(self):
        sdk, alias, stub, unversioned = self.sdk_layout()
        system = sdk / "usr/lib/system/libsystem_c.tbd"
        system.write_bytes(stub.read_bytes())
        for target in ["aarch64-apple-darwin", "x86_64-apple-darwin"]:
            for selected in [sdk, alias]:
                with self.subTest(target=target, sdk=selected):
                    self.native_collect(target, self.sdk_native_run(
                        selected, [unversioned, system],
                        [alias / "usr/lib/libiconv.2.tbd", system]))
                    data = json.loads(Path(str(self.stem) + ".provenance.json").read_text())
                    self.assertEqual(data["sdk"]["resolved"], str(sdk.resolve()))
                    self.assertEqual(len(data["object_inputs"]), 3)
                    self.assertFalse(any(".tbd" in entry for entry in data["object_inputs"]))
                    records = data["sdk_stub_inputs"]
                    self.assertEqual([record["path"] for record in records], [str(unversioned), str(system)])
                    self.assertEqual(records[0]["resolved"], str(stub))
                    self.assertEqual(records[0]["sha256"], provenance.sha256(stub))
                    self.assertEqual(records[0]["trace_paths"], [str(alias / "usr/lib/libiconv.2.tbd")])
                    self.assertIn("not incorporated code", data["limits"])

    def test_darwin_sdk_stubs_fail_closed_without_interface_and_trace_evidence(self):
        sdk, alias, stub, unversioned = self.sdk_layout()
        original = stub.read_bytes()
        outside = self.directory / "outside.tbd"
        outside.write_bytes(original)
        escaped = sdk / "usr/lib/escaped.tbd"
        escaped.symlink_to(outside)
        for failure in ["missing-trace", "argv-only", "trace-suffix", "wrong-sdk", "binary", "nul",
                        "unknown-format", "unknown-version", "escape", "missing-file", "archive", "object", "dylib"]:
            stub.write_bytes(original)
            entry = unversioned
            traces = [alias / "usr/lib/libiconv.2.tbd"]
            if failure in ["missing-trace", "argv-only"]:
                traces = []
            elif failure == "trace-suffix":
                traces = [str(traces[0]) + " extra"]
            elif failure == "wrong-sdk":
                entry = outside
                traces = [outside]
            elif failure == "binary":
                stub.write_bytes(b"\xcf\xfa\xed\xfe\x00binary")
            elif failure == "nul":
                stub.write_bytes(original + b"\x00")
            elif failure == "unknown-format":
                stub.write_bytes(b"arbitrary text masquerading as a stub\n")
            elif failure == "unknown-version":
                stub.write_bytes(original.replace(b"tbd-version:     4", b"tbd-version:     5"))
            elif failure == "escape":
                entry = escaped
                traces = [escaped]
            elif failure == "missing-file":
                entry = sdk / "usr/lib/missing.tbd"
                traces = [entry]
            elif failure in ["archive", "object", "dylib"]:
                entry = sdk / ("usr/lib/unknown" + {"archive": ".a", "object": ".o", "dylib": ".dylib"}[failure])
                entry.write_bytes(original)
                traces = [entry]
            native = self.sdk_native_run(sdk, [entry], traces)

            def effect(argv, **kwargs):
                result = native(argv, **kwargs)
                if failure == "argv-only" and argv[:2] == ["cargo", "rustc"]:
                    result.stdout += f'clang "{unversioned}"\n'
                return result

            for target in ["aarch64-apple-darwin", "x86_64-apple-darwin"]:
                with self.subTest(failure=failure, target=target), self.assertRaisesRegex(
                        RuntimeError, "unreviewed Darwin|unsupported SDK text stub"):
                    self.native_collect(target, effect)
                self.assertFalse(Path(str(self.stem) + ".link.map").exists())
                self.assertFalse(Path(str(self.stem) + ".provenance.json").exists())

    def test_darwin_sdk_fixture_canonicalizes_symlinked_temporary_root(self):
        root_alias = self.directory / "temporary-root-alias"
        root_alias.symlink_to(self.directory.resolve(), target_is_directory=True)
        self.directory = root_alias
        sdk, alias, stub, unversioned = self.sdk_layout()
        self.assertEqual(sdk, sdk.resolve())
        self.assertEqual(sdk.name, "MacOSX.sdk")
        self.assertFalse(sdk.is_symlink())
        self.assertEqual(alias.name, "MacOSX15.5.sdk")
        self.assertTrue(alias.is_symlink())
        self.assertEqual(alias.resolve(), sdk)
        self.assertTrue(unversioned.is_symlink())
        self.assertEqual(unversioned.resolve(), stub)
        for target in ["aarch64-apple-darwin", "x86_64-apple-darwin"]:
            with self.subTest(target=target):
                self.native_collect(target, self.sdk_native_run(
                    alias, [alias / "usr/lib/libiconv.tbd"], [unversioned]))
                data = json.loads(Path(str(self.stem) + ".provenance.json").read_text())
                self.assertEqual(data["sdk"]["resolved"], str(sdk))
                self.assertEqual(data["sdk_stub_inputs"][0]["resolved"], str(stub))

    def test_darwin_generated_symbols_require_exact_same_link_trace(self):
        for evidence in ["absent", "argv-only", "other-object", "archive-member", "prefix",
                         "diagnostic-member", "diagnostic-suffix", "nested-prefix", "wrong-prefix", "indented-prefix"]:
            def native(argv, **kwargs):
                result = self.native_run(argv, **kwargs)
                if argv[:2] == ["cargo", "rustc"]:
                    symbols = result.stderr.splitlines()[0].removeprefix("warning: linker stdout: ")
                    result.stderr = {
                        "absent": "", "argv-only": "",
                        "other-object": symbols.replace("rustcYx5yPX", "rustcABC123"),
                        "archive-member": symbols + "(fake.o)",
                        "prefix": "not a trace " + symbols,
                        "diagnostic-member": "warning: linker stdout: " + symbols + "(fake.o)",
                        "diagnostic-suffix": "warning: linker stdout: " + symbols + " extra",
                        "nested-prefix": "not a trace warning: linker stdout: " + symbols,
                        "wrong-prefix": "warning: linker stderr: " + symbols,
                        "indented-prefix": " warning: linker stdout: " + symbols,
                    }[evidence]
                    if evidence != "argv-only":
                        result.stdout = ""
                return result

            with self.subTest(evidence=evidence), self.assertRaisesRegex(RuntimeError, "unreviewed Darwin static input"):
                self.native_collect(effect=native)
            self.assertFalse(Path(str(self.stem) + ".link.map").exists())
            self.assertFalse(Path(str(self.stem) + ".provenance.json").exists())

    def test_darwin_generated_symbols_do_not_allow_other_static_objects(self):
        deps = ROOT / "target/aarch64-apple-darwin/release/deps"
        candidates = [
            deps / "rustc7hS4lE/unknown.o", deps / "unknown.o",
            deps / "other7hS4lE/symbols.o", deps / "rustc12345/symbols.o",
            deps / "rustc1234567/symbols.o", deps / "rustc123_56/symbols.o",
            ROOT / "target/x86_64-apple-darwin/release/deps/rustc7hS4lE/symbols.o",
            ROOT / "target/aarch64-apple-darwin/release/rustc7hS4lE/symbols.o",
            self.directory / "rustc7hS4lE/symbols.o",
            str(deps / "rustc7hS4lE/symbols.o") + "(fake.o)",
            "/SDK/libunreviewed.a(member.o)",
        ]
        for candidate in candidates:
            def native(argv, **kwargs):
                result = self.native_run(argv, **kwargs)
                if argv[:2] == ["cargo", "rustc"]:
                    Path(str(self.stem) + ".link.map").write_text(
                        f"# Object files:\n[0] linker synthesized\n[1] {candidate}\n# Sections:\n")
                    result.stdout = str(candidate) + "\n"
                    result.stderr = ""
                return result

            with self.subTest(candidate=candidate), self.assertRaisesRegex(RuntimeError, "unreviewed Darwin static input"):
                self.native_collect(effect=native)
            self.assertFalse(Path(str(self.stem) + ".link.map").exists())
            self.assertFalse(Path(str(self.stem) + ".provenance.json").exists())

    def test_darwin_generated_symbols_reject_symlink_escape(self):
        target = "aarch64-apple-darwin"
        alias = self.directory / "root-alias"
        alias.symlink_to(self.directory, target_is_directory=True)
        self.assertNotEqual(alias, alias.resolve())
        for fixture_root in [self.directory, alias]:
            with self.subTest(root=fixture_root):
                root = fixture_root.resolve()
                deps = root / "target" / target / "release/deps"
                deps.mkdir(parents=True, exist_ok=True)
                symbols = deps / "rustc7hS4lE/symbols.o"
                text = f"# Object files:\n[0] linker synthesized\n[1] {symbols}\n# Sections:\n"
                self.assertEqual(provenance.darwin_objects(
                    text, root, root / "rust", target, str(symbols)),
                     (["linker synthesized", str(symbols)], []))
                symbols.parent.symlink_to(root, target_is_directory=True)
                with self.assertRaisesRegex(RuntimeError, "unreviewed Darwin static input"):
                    provenance.darwin_objects(text, root, root / "rust", target, str(symbols))
                symbols.parent.unlink()

    def test_darwin_preserves_opaque_symbol_bytes_in_normalized_map(self):
        map_path = Path(str(self.stem) + ".link.map")
        for target in ["aarch64-apple-darwin", "x86_64-apple-darwin"]:
            linked = {}

            def native(argv, **kwargs):
                result = self.native_run(argv, **kwargs)
                if argv[:2] == ["cargo", "rustc"]:
                    linked["map"] = (map_path.read_bytes() + b"0x1000 0x10 [  1] _opaque_\xc0\xff\x80_"
                                     + str(ROOT / "src/main.rs").encode("utf-8")
                                     + "_caf\u00e9\n".encode("utf-8"))
                    map_path.write_bytes(linked["map"])
                    linked["binary"] = self.binary.read_bytes()
                return result

            with self.subTest(target=target):
                self.native_collect(target, effect=native)
                retained = map_path.read_bytes()
                self.assertEqual(retained, linked["map"].replace(str(ROOT).encode("utf-8"), b"<repo>"))
                data = json.loads(Path(str(self.stem) + ".provenance.json").read_text())
                self.assertEqual(data["map"]["sha256"], hashlib.sha256(retained).hexdigest())
                self.assertEqual(self.binary.read_bytes(), linked["binary"])
                self.assertEqual(data["binary"]["sha256"], hashlib.sha256(linked["binary"]).hexdigest())

    def test_darwin_opaque_symbols_do_not_relax_object_validation(self):
        map_path = Path(str(self.stem) + ".link.map")
        for entry, error in [
            (b"invalid object \xc0", "unrecognized Apple object entry"),
            (b"[1] /SDK/libunreviewed.a(member.o)", "unreviewed Darwin static input"),
            (b"[1] /SDK/../SDK/libunreviewed\xc0.a(member.o)", "unreviewed Darwin static input"),
        ]:
            def native(argv, **kwargs):
                result = self.native_run(argv, **kwargs)
                if argv[:2] == ["cargo", "rustc"]:
                    map_path.write_bytes(b"# Object files:\n[0] linker synthesized\n" + entry
                                         + b"\n# Sections:\n# Symbols:\n0x1000 0x10 [1] _opaque_\xc0\xff\n")
                return result

            with self.subTest(entry=entry):
                with self.assertRaisesRegex(RuntimeError, error) as failure:
                    self.native_collect(effect=native)
                if error == "unreviewed Darwin static input":
                    name = entry.decode("utf-8", errors="surrogateescape").split(" ", 1)[1]
                    self.assertIn(repr(name), str(failure.exception))
                    self.assertIn(repr(str(Path(name.split("(", 1)[0]).resolve())), str(failure.exception))
                self.assertFalse(map_path.exists())
                self.assertFalse(Path(str(self.stem) + ".provenance.json").exists())

    def test_darwin_bad_arch_minimum_signature_dylib_or_static_input_fails_closed(self):
        for failure in ["architecture", "deployment", "signature", "dylib", "static", "verify", "map"]:
            def fail(argv, **kwargs):
                result = self.native_run(argv, **kwargs)
                if failure == "architecture" and argv[0] == "lipo":
                    result.stdout = "x86_64 arm64\n"
                elif failure == "deployment" and argv[:2] == ["otool", "-l"]:
                    result.stdout = result.stdout.replace("15.0", "14.0")
                elif failure == "signature" and argv[:2] == ["codesign", "--display"]:
                    result.stdout = "Signature=Developer ID"
                elif failure == "dylib" and argv[:2] == ["otool", "-L"]:
                    result.stdout += "\t/opt/homebrew/lib/unreviewed.dylib (compatibility version 1)\n"
                elif failure == "verify" and argv[:2] == ["codesign", "--verify"]:
                    raise subprocess.CalledProcessError(1, argv)
                elif failure in ["static", "map"] and argv[:2] == ["cargo", "rustc"]:
                    path = Path(str(self.stem) + ".link.map")
                    path.write_text("unknown format" if failure == "map" else
                                    "# Object files:\n[0] linker synthesized\n[1] /SDK/libunreviewed.a(member.o)\n# Sections:\n")
                return result
            with self.subTest(failure=failure), self.assertRaises((RuntimeError, subprocess.CalledProcessError)):
                self.native_collect(effect=fail)
            self.assertFalse(Path(str(self.stem) + ".provenance.json").exists())
            self.assertFalse(Path(str(self.stem) + ".link.map").exists())

    def test_darwin_translated_or_wrong_toolchain_host_never_links(self):
        for failure in ["translated", "version", "host", "old-os"]:
            self.native_collect()
            self.calls.clear()
            def fail(argv, **kwargs):
                result = self.native_run(argv, **kwargs)
                if failure == "translated" and argv[0] == "sysctl":
                    result.stdout = "1\n"
                if argv == ["rustc", "-vV"]:
                    if failure == "version":
                        result.stdout = result.stdout.replace("1.99.0", "1.99.1")
                    elif failure == "host":
                        result.stdout = result.stdout.replace("aarch64-apple-darwin", "x86_64-apple-darwin")
                if failure == "old-os" and argv == ["sw_vers", "-productVersion"]:
                    result.stdout = "14.7\n"
                return result
            with self.subTest(failure=failure), self.assertRaises(RuntimeError):
                self.native_collect(effect=fail)
            self.assertFalse(any(c[:2] == ["cargo", "rustc"] for c in self.calls))
            self.assertFalse(Path(str(self.stem) + ".provenance.json").exists(), "preflight failure must remove stale success evidence")
            self.assertFalse(Path(str(self.stem) + ".link.map").exists())

    def test_darwin_unreviewed_dylib_error_identifies_native_install_names(self):
        def native(argv, **kwargs):
            result = self.native_run(argv, **kwargs)
            if argv[:2] == ["otool", "-L"]:
                result.stdout += "\t/opt/homebrew/lib/unreviewed.dylib (compatibility version 1)\n"
            return result
        with self.assertRaisesRegex(RuntimeError, "/opt/homebrew/lib/unreviewed.dylib"):
            self.native_collect(effect=native)
        self.assertFalse(Path(str(self.stem) + ".provenance.json").exists())

    def test_darwin_system_iconv_is_recorded_as_external_library(self):
        def native(argv, **kwargs):
            result = self.native_run(argv, **kwargs)
            if argv[:2] == ["otool", "-L"]:
                result.stdout += "\t/usr/lib/libiconv.2.dylib (compatibility version 7.0.0)\n"
            return result
        self.native_collect(effect=native)
        data = json.loads(Path(str(self.stem) + ".provenance.json").read_text())
        self.assertEqual(data["dylibs"], ["/usr/lib/libSystem.B.dylib", "/usr/lib/libiconv.2.dylib"])

    def test_darwin_requires_native_host_before_any_link(self):
        with patch.object(provenance, "run", return_value=subprocess.CompletedProcess([], 0, "Linux\n", "")) as command:
            with self.assertRaisesRegex(RuntimeError, "native"):
                provenance.collect("aarch64-apple-darwin", self.binary, self.stem)
        self.assertFalse(any(c.args[0][:2] == ["cargo", "rustc"] for c in command.call_args_list))

    def test_darwin_kernel_evidence_does_not_collect_private_hostname(self):
        def native(argv, **kwargs):
            result = self.native_run(argv, **kwargs)
            if argv == ["uname", "-a"]:
                result.stdout = "Darwin personal-host.example.invalid 24.6.0 arm64\n"
            elif argv == ["uname", "-r"]:
                result.stdout = "24.6.0\n"
            return result
        self.native_collect(effect=native)
        evidence = Path(str(self.stem) + ".provenance.json").read_text()
        self.assertNotIn("personal-host.example.invalid", evidence)
        self.assertEqual(json.loads(evidence)["versions"]["kernel"], "24.6.0\n")

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

    def test_absolute_gcc_verbose_command_is_not_a_trace_filename(self):
        command = (
            "/home/runner/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/"
            "lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld.lld "
            "-plugin /usr/libexec/gcc/x86_64-linux-gnu/13/liblto_plugin.so "
            "-plugin-opt=/usr/libexec/gcc/x86_64-linux-gnu/13/lto-wrapper "
            "-plugin-opt=-fresolution=/tmp/cc123.res --build-id "
            "-o /home/runner/work/herdr-crossroads/target/release/deps/herdr_crossroads"
        )
        traces = (f"         {command}\n"
                  "         /usr/lib/gcc/x86_64-linux-gnu/13/crtbeginS.o\n"
                  "         /usr/lib/libgcc.a(runtime member.o)\n")
        self.assertEqual(provenance.traced_paths(traces), [
            "/usr/lib/gcc/x86_64-linux-gnu/13/crtbeginS.o", "/usr/lib/libgcc.a",
        ], "verbose linker commands must remain raw evidence, never file records")

    def test_ambiguous_whitespace_paths_remain_unindexed(self):
        self.assertEqual(provenance.traced_paths(
            '/absolute/path with spaces.o\n/absolute/path\twith-tab.o\n'
            '/absolute/path\u00a0with-space.o\n/absolute/"quoted".o\n'
            '/usr/lib/Scrt1.o\n'), ["/usr/lib/Scrt1.o"])

    def test_collection_retains_verbose_command_without_indexing_it(self):
        command = f"{Path.home()}/.rustup/toolchains/fixture/bin/gcc-ld/ld.lld -plugin /usr/libexec/gcc/13/liblto_plugin.so"

        def verbose_run(argv, **kwargs):
            result = self.fake_run(argv, **kwargs)
            if argv[:2] == ["cargo", "rustc"]:
                result.stderr = f"         {command}\n" + result.stderr
            return result

        with patch.object(provenance, "run", side_effect=verbose_run):
            provenance.collect("x86_64-unknown-linux-gnu", self.binary, self.stem)
        data = json.loads(Path(str(self.stem) + ".provenance.json").read_text())
        self.assertEqual([row["path"] for row in data["trace_files"]],
                         sorted([str(self.startup), str(self.archive)]))
        self.assertIn("<rustup>/toolchains/fixture/bin/gcc-ld/ld.lld -plugin /usr/libexec/gcc/13/liblto_plugin.so",
                      data["link_diagnostics"])

    def test_failure_diagnostic_normalizes_private_paths_and_argv(self):
        candidate = f"{Path.home()}/.rustup/toolchains/fixture/bin/ld.lld -plugin /usr/libexec/gcc/13/liblto_plugin.so"
        for error, status in [
            (OSError(36, "File name too long", candidate), 1),
            (subprocess.CalledProcessError(19, ["gcc", candidate, str(ROOT / "target/input.o")]), 19),
        ]:
            with self.subTest(error=type(error).__name__):
                stderr = io.StringIO()
                with patch.object(subprocess, "run", side_effect=error), \
                        patch.object(sys, "argv", [str(SPEC.origin), "x86_64-unknown-linux-gnu", str(self.binary), str(self.stem)]), \
                        redirect_stderr(stderr), self.assertRaises(SystemExit) as failure:
                    runpy.run_path(str(SPEC.origin), run_name="__main__")
                self.assertEqual(failure.exception.code, status)
                self.assertNotIn(str(Path.home()), stderr.getvalue())
                self.assertNotIn(str(ROOT), stderr.getvalue())
                self.assertIn("<rustup>/toolchains/fixture/bin/ld.lld", stderr.getvalue())
                self.assertIn("/usr/libexec/gcc/13/liblto_plugin.so", stderr.getvalue())

    def test_file_stat_errors_are_not_discarded(self):
        real_stat = os.stat
        for error in [PermissionError(errno.EACCES, "denied", str(self.startup)),
                      OSError(errno.EIO, "I/O error", str(self.startup))]:
            with self.subTest(errno=error.errno):
                def faulty_stat(path, *args, **kwargs):
                    if os.fspath(path) == str(self.startup):
                        raise error
                    return real_stat(path, *args, **kwargs)

                with patch.object(os, "stat", side_effect=faulty_stat), \
                        patch.object(provenance, "run") as command:
                    with self.assertRaises(type(error)) as failure:
                        provenance.file_record(self.startup)
                self.assertIs(failure.exception, error)
                command.assert_not_called()

    def test_nonregular_input_fails_collection(self):
        with patch.object(provenance, "run") as command:
            with self.assertRaisesRegex(RuntimeError, "not a regular file"):
                provenance.file_record(self.directory)
        command.assert_not_called()

    def test_symlink_records_original_and_resolved_regular_file(self):
        alias = self.directory / "input-alias"
        alias.symlink_to(self.directory, target_is_directory=True)
        self.startup = alias / self.startup.name
        link = self.directory / "startup-link.o"
        link.symlink_to(self.startup)
        with patch.object(provenance, "run", return_value=subprocess.CompletedProcess([], 1, "", "no owner")) as command:
            record = provenance.file_record(link)
        self.assertEqual(record["path"], str(link))
        self.assertEqual(record["resolved"], str(self.startup.resolve()))
        self.assertEqual(record["sha256"], provenance.sha256(self.startup))
        self.assertEqual([call.args[0] for call in command.call_args_list], [
            ["dpkg-query", "-S", str(link)], ["dpkg-query", "-S", str(self.startup.resolve())],
        ])

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
