"""Portable GNU evidence contracts; these fixtures are not native execution."""
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
APPROVED_PREPARATION_IMPORTS = (
    "bcryptprimitives.dll",
    "api-ms-win-crt-environment-l1-1-0.dll",
    "api-ms-win-crt-heap-l1-1-0.dll",
    "api-ms-win-crt-locale-l1-1-0.dll",
    "api-ms-win-crt-math-l1-1-0.dll",
    "api-ms-win-crt-private-l1-1-0.dll",
    "api-ms-win-crt-runtime-l1-1-0.dll",
    "api-ms-win-crt-stdio-l1-1-0.dll",
    "api-ms-win-crt-string-l1-1-0.dll",
)


def linker_message(root, trace):
    # Rust 1.99 report_linker_output: UTF-8 stdout is one multiline lint
    # message with trim_end(), not child notes or raw Cargo stdout.
    return {"reason": "compiler-message", "manifest_path": str(root / "Cargo.toml"),
            "package_id": f"path+file://{root}#herdr-crossroads@0.3.0",
            "target": {"name": "herdr-crossroads", "kind": ["bin"], "crate_types": ["bin"],
                       "src_path": str(root / "src/main.rs"), "edition": "2021",
                       "doc": True, "doctest": False, "test": True},
            "message": {"rendered": "\x1b[1mwarning\x1b[0m: linker stdout: " + trace.decode("utf-8").rstrip() + "\n",
                        "message": "linker stdout: " + trace.decode("utf-8").rstrip(),
                        "code": {"code": "linker_messages", "explanation": None},
                        "level": "warning", "spans": [], "children": []}}


class WindowsProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.module = runpy.run_path(str(ROOT / "scripts/build-windows.py"))

    def collect(self, failure=None, newline="\n", synthetic=False, pe_text=None, pe_stderr=b""):
        builder = self.module["build"]
        run_command = self.module["run"]
        captured = {}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            mingw = root / "mingw"
            mingw.mkdir()
            libraries = [mingw / n for n in ("crt2.o", "crtbegin.o", "crtend.o", "libmingw32.a", "libmingwex.a", "libmsvcrt.a", "libgcc.a", "libgcc_eh.a", "libpthread.a")]
            if synthetic:
                libraries.extend(mingw / f"input-{number}.o" for number in range(94))
            for p in libraries:
                p.write_bytes(p.name.encode())
            trace = ("\n".join(map(str, libraries)) + f"\n({libraries[3]})part.o\n").encode()
            raw_map = ("Linker script and memory map\n" + "\n".join(f"LOAD {p}" for p in libraries) + "\nOUTPUT(app.exe pei-x86-64)\n").encode()
            if synthetic:
                raw_map += b"LOAD dll stuff\n"
            if failure == "duplicate_synthetic":
                raw_map += b"LOAD dll stuff\nLOAD dll stuff\n"
            trace = trace.replace(b"\n", newline.encode())
            raw_map = raw_map.replace(b"\n", newline.encode())
            binary = root / "target/x86_64-pc-windows-gnu/release/herdr-crossroads.exe"
            binary.parent.mkdir(parents=True)
            for name in ("LICENSE", "THIRD-PARTY-NOTICES.md"):
                (root / name).write_bytes(name.encode())
            stem = root / "target/builds/herdr-crossroads-0.3.0-x86_64-pc-windows-gnu"
            evidence = Path(str(stem) + ".provenance.json")
            map_path = Path(str(stem) + ".link.map")
            calls = []

            def command(args, **kwargs):
                calls.append((args, kwargs))
                if args[:2] == ["rustc", "-vV"]:
                    return "rustc 1.99.0 fixture\nhost: x86_64-pc-windows-gnu\n"
                if args[:2] == ["cargo", "--version"]:
                    return "cargo 1.99.0 fixture\n"
                if args[0].endswith("objdump.exe"):
                    if failure == "collect_binary":
                        binary.write_bytes(b"mutated")
                    text = pe_text if pe_text is not None else f"{binary}: file format pei-x86-64\nDLL Name: KERNEL32.dll\n"
                    def inspect(arguments, **options):
                        captured["pe_capture_limit"] = options.get("capture_limit", 1024 * 1024)
                        limit = captured["pe_capture_limit"]
                        return subprocess.CompletedProcess(arguments, 0, text.encode()[:limit], pe_stderr[:limit])
                    with patch.dict(builder.__globals__, supervise=inspect):
                        return run_command(args, **kwargs)
                if args[:3] == ["git", "rev-parse", "HEAD"]:
                    return "f" * 40
                if args[0] == "rustc":
                    captured.update(json.loads(evidence.read_text()))
                    captured["observed_map_sha256"] = hashlib.sha256(map_path.read_bytes()).hexdigest()
                    captured["observed_map"] = map_path.read_bytes()
                    captured["expected_map"] = builder.__globals__["normalize"](raw_map.decode()).encode()
                    captured["expected_trace"] = builder.__globals__["normalize"](trace.rstrip().decode())
                    if failure == "gate":
                        raise subprocess.TimeoutExpired(args, 0.1)
                    if failure == "test_binary":
                        binary.write_bytes(b"mutated")
                    if failure == "test_map":
                        map_path.write_bytes(b"tampered")
                    if failure == "test_evidence":
                        evidence.write_text("{}")
                    if failure == "test_library":
                        libraries[0].write_bytes(b"tampered")
                if args[:3] == ["cargo", "test", "--locked"] and "--no-run" in args:
                    return json.dumps({"reason": "compiler-artifact", "target": {"name": "windows_console"}, "executable": "console.exe"})
                if args[0] == self.module["sys"].executable and args[-1].endswith("install-binary.py"):
                    with tarfile.open(str(stem) + ".tar.gz") as archive:
                        captured["archive_members"] = archive.getnames()
                    if failure in ("installed_binary", "installed_archive"):
                        installed = Path(args[1]).parent.parent / "target/release/herdr-crossroads.exe"
                        installed.parent.mkdir(parents=True)
                        installed.write_bytes(b"fixture PE")
                        return ""
                    raise RuntimeError("stop after packaging")
                if "tests/test_windows_acceptance.py" in args and failure in ("installed_binary", "installed_archive"):
                    installed = Path(kwargs["env"]["CROSSROADS_WINDOWS_BINARY"])
                    if installed != binary:
                        if failure == "installed_binary":
                            installed.write_bytes(b"tampered installed binary")
                        else:
                            Path(str(stem) + ".tar.gz").write_bytes(b"tampered archive")
                return ""

            def link(args, **kwargs):
                captured["link_environment"] = kwargs["env"]
                captured["link_command"] = args
                binary.write_bytes(b"fixture PE")
                if failure != "missing_map":
                    map_path.write_bytes(raw_map if failure != "bad_map_load" else
                                         raw_map + b'LOAD "C:/private-person/secret.a"\n')
                message = linker_message(root, trace)
                output = b"" if failure == "missing_trace" else (json.dumps(message) + '\n' +
                         json.dumps({"reason": "build-finished", "success": True}) + '\n').encode()
                stderr = b"\x1b[32m    Finished\x1b[0m release profile\r\n"
                if failure == "raw_trace":
                    output = trace
                if failure == "stderr_trace":
                    output, stderr = b"", b"warning: linker stdout: " + trace
                if failure == "truncated_stdout":
                    output += b" " * (1024 * 1024 - len(output))
                if failure == "truncated_stderr":
                    stderr = b" " * (1024 * 1024)
                captured["cargo_stdout"] = output
                return subprocess.CompletedProcess(args, 0, output, stderr)

            def source(path):
                if Path(path).name == "install-binary.py":
                    return {"host_target": lambda: "x86_64-pc-windows-gnu", "verify_binary": lambda data, target: None}
                return {"release_version": lambda: "0.3.0"}

            with patch.dict(builder.__globals__, ROOT=root, MINGW=mingw, run=command, supervise=link,
                            toolchain_evidence=lambda: {"executables": []}), patch("os.chdir"), patch(
                    "platform.system", return_value="Windows"), patch("runpy.run_path", side_effect=source), patch("shutil.copy2"):
                try:
                    builder()
                except RuntimeError as error:
                    self.assertEqual(str(error), "stop after packaging")
                    if failure is not None:
                        raise AssertionError("corrupt or missing evidence reached packaging")
                finally:
                    self.assertFalse(list(stem.parent.glob(stem.name + ".*")), "failure removes every selected output")
            captured["raw_map"] = raw_map
            captured["trace"] = trace.rstrip()
            captured["calls"] = calls
        return captured

    def test_collector_binds_binary_map_diagnostics_and_keeps_all_native_gates(self):
        try:
            evidence = self.collect()
        except ValueError as error:
            self.fail(f"successful Rust linker diagnostic must reach packaging: {error}")
        self.assertEqual(evidence["schema_version"], 2)
        self.assertEqual(evidence["resolved_link_inputs"]["synthetic_inputs"], [])
        self.assertEqual(evidence["binary_sha256"], hashlib.sha256(b"fixture PE").hexdigest())
        self.assertEqual(evidence["link_map_raw_sha256"], hashlib.sha256(evidence["raw_map"]).hexdigest())
        self.assertEqual(evidence["resolved_link_inputs"]["trace_raw_sha256"], hashlib.sha256(evidence["trace"]).hexdigest())
        self.assertEqual(evidence["link_diagnostics"]["stdout"]["raw_sha256"], hashlib.sha256(evidence["cargo_stdout"]).hexdigest())
        self.assertEqual(evidence["link_diagnostics"]["stderr"]["raw_sha256"], hashlib.sha256(b"\x1b[32m    Finished\x1b[0m release profile\r\n").hexdigest())
        self.assertEqual(evidence["link_map_sha256"], evidence["observed_map_sha256"])
        self.assertEqual(evidence["pe_import_diagnostics_sha256"], hashlib.sha256(evidence["pe_import_diagnostics"].encode()).hexdigest())
        self.assertTrue(evidence["preparation_only"])
        self.assertFalse(evidence["distribution_approved"])
        self.assertEqual(evidence["link_environment"]["RUSTUP_TOOLCHAIN"], "1.99.0-x86_64-pc-windows-gnu")
        self.assertIn("link-self-contained=no", evidence["link_environment"]["CARGO_ENCODED_RUSTFLAGS"])
        self.assertIn("--message-format=json", evidence["link_command"])
        self.assertIn("linker-messages", evidence["link_command"])
        self.assertIn("not original linker bytes", evidence["resolved_link_inputs"]["trace_capture"])
        groups = [args for args, kw in evidence["calls"] if args[:3] == ["cargo", "test", "--locked"]]
        self.assertEqual(len(groups), 10)
        self.assertTrue(all(kw["timeout"] == 60 for args, kw in evidence["calls"] if args in groups and "--no-run" not in args))

    def test_large_pe_capture_parses_tail_and_hashes_complete_output(self):
        text = "app.exe: file format pei-x86-64\n" + " " * 1940764 + "\nDLL Name: KERNEL32.dll\n"
        evidence = self.collect(pe_text=text)
        self.assertEqual(evidence["pe_capture_limit"], 4 * 1024 * 1024)
        self.assertEqual(evidence["pe_imports"], ["kernel32.dll"])
        self.assertEqual(evidence["pe_import_diagnostics"], text)
        self.assertEqual(evidence["pe_import_captured_text_sha256"], hashlib.sha256(text.encode()).hexdigest())
        self.assertEqual(evidence["pe_import_diagnostics_sha256"], hashlib.sha256(text.encode()).hexdigest())
        for args, options in evidence["calls"]:
            self.assertEqual("capture_limit" in options, args[0].endswith("objdump.exe"))

    def test_large_pe_tail_imports_and_capture_boundary_fail_closed_with_cleanup(self):
        prefix = "app.exe: file format pei-x86-64\nDLL Name: KERNEL32.dll\n" + " " * (1024 * 1024) + "\n"
        for tail, reason in (("DLL Name: foreign.dll\n", "unreviewed PE DLL import"),
                             ("Entry d 000000001000 00000010 Delay Import Directory\n", "delay imports")):
            with self.subTest(tail=tail), self.assertRaisesRegex(ValueError, reason):
                self.collect(pe_text=prefix + tail)
        for size in (4 * 1024 * 1024, 4 * 1024 * 1024 + 1):
            for stream in ("stdout", "stderr"):
                options = {"pe_text": " " * size} if stream == "stdout" else {"pe_stderr": b" " * size}
                with self.subTest(size=size, stream=stream), self.assertRaisesRegex(ValueError, "truncated"):
                    self.collect(**options)

    def test_other_command_capture_still_rejects_one_mib_on_either_stream(self):
        command = self.module["run"]
        for stdout, stderr in ((b"x" * (1024 * 1024), b""), (b"", b"y" * (1024 * 1024))):
            def supervise(args, **options):
                self.assertEqual(options.get("capture_limit", 1024 * 1024), 1024 * 1024)
                return subprocess.CompletedProcess(args, 0, stdout, stderr)
            with patch.dict(command.__globals__, supervise=supervise), self.assertRaisesRegex(ValueError, "truncated"):
                command(["cargo", "--version"], capture=True)

    def test_mutation_missing_trace_map_and_gate_failure_never_leave_stale_evidence(self):
        cases = {"collect_binary": "binary changed", "test_binary": "binary changed", "test_map": "map changed",
                 "test_evidence": "provenance changed", "test_library": "input changed", "installed_binary": "installed binary changed",
                  "installed_archive": "archive changed", "missing_trace": "linker stdout diagnostic", "missing_map": "link.map", "gate": "timed out",
                  "raw_trace": "invalid Cargo JSON", "stderr_trace": "linker stdout diagnostic",
                   "bad_map_load": "unsupported GNU map LOAD at line 12",
                   "duplicate_synthetic": "unsupported GNU map LOAD at line 13",
                  "truncated_stdout": "truncated", "truncated_stderr": "truncated"}
        for failure, reason in cases.items():
            with self.subTest(failure=failure), self.assertRaisesRegex((ValueError, FileNotFoundError, subprocess.TimeoutExpired), reason):
                self.collect(failure)

    def test_collector_accepts_lf_and_crlf_without_normalizing_evidence_newlines(self):
        for newline in ("\n", "\r\n"):
            with self.subTest(newline=newline):
                evidence = self.collect(newline=newline)
                self.assertEqual(evidence["observed_map"].decode().splitlines()[-1], "OUTPUT(app.exe pei-x86-64)")
                self.assertEqual(evidence["observed_map"], evidence["expected_map"])
                self.assertEqual(evidence["resolved_link_inputs"]["trace"], evidence["expected_trace"])
                self.assertEqual(evidence["raw_map"].count(newline.encode()), 11)
                self.assertEqual(evidence["observed_map"].count(newline.encode()), 11)
                self.assertEqual(evidence["link_map_raw_sha256"], hashlib.sha256(evidence["raw_map"]).hexdigest())
                self.assertEqual(evidence["link_map_sha256"], hashlib.sha256(evidence["observed_map"]).hexdigest())
                self.assertEqual(evidence["resolved_link_inputs"]["trace_raw_sha256"], hashlib.sha256(evidence["trace"]).hexdigest())
                self.assertEqual(evidence["resolved_link_inputs"]["trace"].count(newline), 9)
                self.assertEqual(evidence["link_diagnostics"]["stdout"]["raw_sha256"], hashlib.sha256(evidence["cargo_stdout"]).hexdigest())

    def test_exact_map_only_synthetic_input_keeps_real_inventory_and_raw_hashes(self):
        for newline in ("\n", "\r\n"):
            with self.subTest(newline=newline):
                try:
                    evidence = self.collect(newline=newline, synthetic=True)
                except ValueError as error:
                    self.fail(f"exact GNU synthetic map input must reach packaging: {error}")
                inputs = evidence["resolved_link_inputs"]
                self.assertEqual(evidence["schema_version"], 2)
                self.assertTrue(evidence["preparation_only"])
                self.assertFalse(evidence["distribution_approved"])
                self.assertEqual(inputs["synthetic_inputs"], [
                    {"name": "dll stuff", "kind": "gnu_pe_dll_filler", "map_line": 106}])
                self.assertEqual(len(inputs["files"]), 103)
                self.assertTrue(all(record["availability"] == "read_after_link" and "sha256" in record
                                    for record in inputs["files"]))
                self.assertEqual(len(inputs["archive_members"]), 1)
                self.assertNotIn("dll stuff", inputs["trace"])
                self.assertEqual(inputs["trace"], evidence["expected_trace"])
                self.assertEqual(inputs["trace_raw_sha256"], hashlib.sha256(evidence["trace"]).hexdigest())
                self.assertEqual(evidence["raw_map"].count(b"LOAD "), 104)
                self.assertEqual(evidence["raw_map"].splitlines()[105], b"LOAD dll stuff")
                self.assertEqual(evidence["link_map_raw_sha256"], hashlib.sha256(evidence["raw_map"]).hexdigest())
                self.assertEqual(evidence["observed_map"], evidence["expected_map"])
                self.assertEqual(evidence["link_map_sha256"], hashlib.sha256(evidence["observed_map"]).hexdigest())

    def test_synthetic_map_exception_does_not_relax_trace_or_real_inventory(self):
        parse = self.module["resolved_link_inputs"]
        trace = b"C:/safe/input.o\n"
        link_map = b"Linker script and memory map\nLOAD C:/safe/input.o\nLOAD dll stuff\nOUTPUT(app.exe pei-x86-64)\n"
        for bad in (b"dll stuff", b"C:/private-person/dll stuff"):
            with self.subTest(trace=bad), self.assertRaisesRegex(ValueError, "unsupported GNU trace at line 2"):
                parse(trace + bad, link_map + b"LOAD private-invalid\n")
        with tempfile.TemporaryDirectory() as temporary:
            collision = Path(temporary) / "dll stuff"
            collision.write_bytes(b"real file, not a synthetic BFD")
            for bad in (collision.name, str(collision)):
                with self.subTest(existing_file=bad), self.assertRaisesRegex(ValueError, "unsupported GNU trace at line 2"):
                    parse(trace + bad.encode(), link_map)
        variants = (b"DLL stuff", b"dll Stuff", b"dll  stuff", b" dll stuff", b"dll stuff ",
                    b"dll\tstuff", b"dll stuff\t", b"'dll stuff'", b'"dll stuff"', b"dll stuff.o",
                    b"dll stuff/private.o", b"-ldll stuff", b"dll stuff(member.o)", b"relative.a",
                    b"private unknown", b"dll stuff\x00", b"dll stuff\vprivate", b"dll stuff\rprivate")
        for bad in variants:
            with self.subTest(map_value=bad), self.assertRaises(ValueError) as caught:
                parse(trace, link_map.replace(b"LOAD dll stuff", b"LOAD " + bad))
            message = str(caught.exception)
            self.assertIn("unsupported GNU map LOAD at line 3; diagnostics=", message)
            self.assertNotIn("private", message)
            self.assertNotIn("dll", message)
        with self.assertRaises(ValueError) as caught:
            parse(trace, link_map + b"LOAD dll stuff\n")
        diagnostic = json.loads(str(caught.exception).split("; diagnostics=", 1)[1])
        self.assertEqual(diagnostic["reason"], "duplicate_synthetic")
        self.assertEqual(diagnostic["line"], 5)
        for broken_trace, broken_map in (
                (trace, link_map + b"LOAD C:/safe/untraced.o\n"),
                (trace + b"C:/safe/unmapped.o\n", link_map),
                (trace, link_map.replace(b"LOAD C:/safe/input.o\n", b"")),
                (trace, link_map + b"LOAD C:/safe/input.o(member.o)\n")):
            with self.subTest(trace=broken_trace, map=broken_map), self.assertRaisesRegex(ValueError, "map LOAD records"):
                parse(broken_trace, broken_map)

    def test_json_trace_preserves_utf8_crlf_windows_escapes_and_member_boundaries(self):
        extract = self.module["linker_trace"]
        trace = 'C:\\mingw64\\path with spaces\\é.o\r\n(C:/mingw64/lib.a)member.o\r\n'
        record = linker_message(ROOT, trace.encode())
        record["message"]["rendered"] = "\x1b[31mnot evidence\x1b[0m"
        raw = (json.dumps(record) + '\n').encode()
        self.assertIn(b'\\r\\n', raw)
        self.assertIn(b'\\\\mingw64', raw)
        self.assertEqual(extract(raw, b"\x1b[32mFinished\x1b[0m\r\n"), trace.rstrip().encode())
        self.assertEqual([self.module["trace_path"](line) for line in extract(raw, b"").decode().splitlines()],
                         [('C:\\mingw64\\path with spaces\\é.o', None), ('C:/mingw64/lib.a', 'member.o')])

    def test_only_one_final_target_stdout_lint_is_trace_evidence(self):
        extract = self.module["linker_trace"]
        for field, value in [("reason", "build-script-executed"), ("manifest_path", "/elsewhere/Cargo.toml"),
                             ("target", {"name": "herdr-crossroads", "kind": ["lib"]}),
                             ("target", {"name": "dependency", "kind": ["bin"]})]:
            record = linker_message(ROOT, b"C:/mingw64/crt2.o\n")
            record[field] = value
            with self.subTest(field=field, value=value), self.assertRaisesRegex(ValueError, "linker stdout diagnostic"):
                extract((json.dumps(record) + '\n').encode(), b"")
        for field, value in [("code", {"code": "unrelated_warning"}), ("code", None), ("level", "note"),
                             ("message", "linker stderr: C:/mingw64/crt2.o"),
                             ("message", "Non-UTF-8 output: C:/mingw64/crt2.o"), ("message", "linker stdout: ")]:
            record = linker_message(ROOT, b"C:/mingw64/crt2.o\n")
            record["message"][field] = value
            with self.subTest(field=field, value=value), self.assertRaisesRegex(ValueError, "linker stdout diagnostic"):
                extract((json.dumps(record) + '\n').encode(), b"")
        record = linker_message(ROOT, b"C:/mingw64/crt2.o\n")
        record["message"]["message"] = "unrelated"
        record["message"]["children"] = [{"message": "linker stdout: C:/mingw64/crt2.o"}]
        with self.assertRaisesRegex(ValueError, "linker stdout diagnostic"):
            extract(json.dumps(record).encode(), b"")
        raw = (json.dumps(linker_message(ROOT, b"C:/mingw64/crt2.o\n")) + '\n').encode()
        with self.assertRaisesRegex(ValueError, "exactly one"):
            extract(raw + raw, b"")
        for broken in (raw[:-10], b"\x1b[32m" + raw, b"\xff", b"[]\n", b"private malformed JSON"):
            with self.subTest(raw=broken[:20]), self.assertRaises(ValueError) as caught:
                extract(broken, b"")
            self.assertEqual(str(caught.exception), "invalid Cargo JSON diagnostic capture")

    def test_json_transport_does_not_relax_trace_validation(self):
        for trace in (b"\x1b[32mC:/mingw64/crt2.o\x1b[0m", b"Non-UTF-8 output: C:/mingw64/crt2.o",
                      b"C:/mingw64/lib.a(member.o) extra", b"warning: C:/mingw64/crt2.o"):
            raw = json.dumps(linker_message(ROOT, trace)).encode()
            with self.subTest(trace=trace), self.assertRaisesRegex(ValueError, "unsupported GNU trace at line 1"):
                self.module["resolved_link_inputs"](self.module["linker_trace"](raw, b""), b"unused map")

    def test_rejection_never_discloses_unknown_paths_or_control_values(self):
        parse = self.module["resolved_link_inputs"]
        for bad in ("C:private-person/secret.o", "@private-token", "secret\t\x1b\x00", "secret" * 10000):
            with self.subTest(bad=bad[:20]), self.assertRaises(ValueError) as caught:
                parse(("\n" + bad).encode(), b"bad map")
            self.assertEqual(str(caught.exception), "unsupported GNU trace at line 2")

    def test_normalization_covers_personal_roots_without_guessing_consumption(self):
        normalize = self.module["normalize"]
        with patch.object(Path, "home", return_value=Path(r"C:\Users\private-user")), patch.dict(
                "os.environ", CARGO_HOME="D:/private-cargo", RUSTUP_HOME="D:/private-rustup"):
            self.assertEqual(normalize(r"C:\Users\private-user\a D:/private-cargo/b D:/private-rustup/c"),
                             r"<home>\a <cargo>/b <rustup>/c")

    def test_map_rejection_reports_physical_line_and_all_load_shapes_privately(self):
        parse = self.module["resolved_link_inputs"]
        cases = [('"C:/private-person/secret.a"', "quote"),
                 ('private-person/secret.a', "relative"),
                 ('C:/private-person/secret.unknown', "unknown_extension"),
                 ('C:/private-person/secret\x00.o', "control"),
                 ('C:/private-person/secret\t\x1b.o', "control"),
                 ('C:/private-person/secret.a(../secret.o)', "member"),
                 ('(C:/private-person/secret.a', "member"),
                 ('-lprivate-person', "relative"),
                 ('C:/private-person/' + 'secret' * 10000, "unknown_extension")]
        for newline in ('\n', '\r\n'):
            for bad, flag in cases:
                raw_map = (newline.join(['Linker script and memory map', '', 'LOAD C:/safe/input.o',
                                         'LOAD ' + bad, 'LOAD D:/other/name.rlib', 'LOAD relative.a',
                                         'OUTPUT(app.exe pei-x86-64)', ''])).encode()
                original = raw_map[:]
                with self.subTest(newline=newline, flag=flag), self.assertRaises(ValueError) as caught:
                    parse(b'C:/safe/input.o', raw_map)
                message = str(caught.exception)
                self.assertIn('unsupported GNU map LOAD at line 4; diagnostics=', message)
                diagnostic = json.loads(message.split('; diagnostics=', 1)[1])
                self.assertEqual(diagnostic['line'], 4)
                self.assertIn(diagnostic['reason'], ('path_or_member', 'archive_syntax'))
                self.assertIn(flag, diagnostic['form'])
                self.assertLessEqual(len(diagnostic['shape']), 120)
                self.assertEqual(diagnostic['load_records'], 4)
                self.assertEqual(sum(diagnostic['forms'].values()) + diagnostic['omitted_records'], 4)
                self.assertTrue(any('relative' in form for form in diagnostic['forms']))
                self.assertTrue(any('extension_rlib' in form for form in diagnostic['forms']))
                self.assertLessEqual(len(message.split('; diagnostics=', 1)[1]), 2048)
                for private in ('private-person', 'secret', '.unknown', 'other', 'input', 'safe', '\x00', '\x1b'):
                    self.assertNotIn(private, message)
                self.assertEqual(raw_map, original)

    def test_map_shape_summary_is_bounded_and_accounts_for_overflow(self):
        parse = self.module['resolved_link_inputs']
        records = ['LOAD relative.a'] + ['LOAD C:/' + ''.join('private' + ('/' if bit == '0' else '\\')
                                                           for bit in f'{count:07b}') + 'secret.o'
                                        for count in range(1, 100)]
        raw_map = ('\n'.join(records) + '\n').encode()
        with self.assertRaises(ValueError) as caught:
            parse(b'C:/safe/input.o', raw_map)
        message = str(caught.exception)
        self.assertIn('; diagnostics=', message)
        payload = message.split('; diagnostics=', 1)[1]
        diagnostic = json.loads(payload)
        self.assertLessEqual(len(payload), 2048)
        self.assertLessEqual(len(diagnostic['forms']), 16)
        self.assertEqual(diagnostic['load_records'], 100)
        self.assertEqual(sum(diagnostic['forms'].values()) + diagnostic['omitted_records'], 100)
        self.assertGreater(diagnostic['omitted_records'], 0)
        self.assertNotIn('private', message)
        self.assertNotIn('secret', message)

    def test_map_diagnostic_shape_masks_names_and_keeps_only_structure(self):
        parse = self.module['resolved_link_inputs']
        for bad, shape in [('"C:/Users/private-person/secret.a"', '"?:/?/?/?.a"'),
                           ('"C:/private-person/secret.rlıb"', '"?:/?/?"'),
                           ('-lprivate-person', '?'),
                           ('C:/private-person/secret.a(../secret.o)', '?:/?/?.a(?/?.o)'),
                           ('"' * 10000 + 'secret', '"' * 120)]:
            with self.subTest(shape=shape), self.assertRaises(ValueError) as caught:
                parse(b'C:/safe/input.o', ('heading\vnot a physical newline\nLOAD ' + bad + '\n').encode())
            payload = str(caught.exception).split('; diagnostics=', 1)[1]
            diagnostic = json.loads(payload)
            self.assertEqual(diagnostic['line'], 2)
            self.assertEqual(diagnostic['shape'], shape)
            self.assertLessEqual(len(payload), 2048)

    def test_full_gnu_host_and_external_linker_for_every_compilation(self):
        self.assertEqual(self.module["TARGET"], "x86_64-pc-windows-gnu")
        source = (ROOT / "scripts/build-windows.py").read_text()
        for expected in ["1.99.0-x86_64-pc-windows-gnu", "link-self-contained=no",
                         "CARGO_ENCODED_RUSTFLAGS", "-Wl,--trace,--trace", "-Wl,-Map,"]:
            self.assertIn(expected, source)
        for forbidden in ["/LINKREPROFULLPATHRSP", "/VERBOSE:LIB", "/MAP:"]:
            self.assertNotIn(forbidden, source)

    def test_trace_map_inputs_have_actual_hashes_and_separate_member_evidence(self):
        parse = self.module.get("resolved_link_inputs")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            mingw = root / "mingw with spaces"
            mingw.mkdir()
            names = ("crt2.o", "crtbegin.o", "crtend.o", "libmingw32.a", "libmingwex.a", "libmsvcrt.a", "libgcc.a", "libgcc_eh.a", "libpthread.a")
            paths = [mingw / name for name in names]
            for path in paths:
                path.write_bytes(path.name.encode())
            missing = root / "removed.rcgu.o"
            trace = "\n".join(map(str, [*paths, missing])) + f"\n({paths[3]})member.o\n"
            link_map = "Linker script and memory map\n" + "\n".join(f"LOAD {p}" for p in [*paths, missing]) + "\nOUTPUT(app.exe pei-x86-64)\n"
            with patch.dict(parse.__globals__, MINGW=mingw, ROOT=root):
                result = parse(trace.encode(), link_map.encode())
            self.assertEqual(result["source"], "GNU ld --trace --trace and map LOAD records")
            self.assertEqual(result["files"][0]["sha256"], hashlib.sha256(b"crt2.o").hexdigest())
            self.assertEqual(result["files"][-1]["availability"], "missing_temporary_after_link")
            self.assertNotIn("sha256", result["files"][-1])
            self.assertEqual(result["archive_members"][0]["member"], "member.o")
            self.assertEqual(result["symbol_contribution_completeness"], "unknown")
            self.assertEqual(result["trace_raw_sha256"], hashlib.sha256(trace.encode()).hexdigest())
            with patch.dict(parse.__globals__, MINGW=mingw, ROOT=root), self.assertRaises(ValueError):
                parse(trace.replace(str(missing), str(root / "unavailable-installed.a")).encode(),
                      link_map.replace(str(missing), str(root / "unavailable-installed.a")).encode())
            for broken, badmap in [(trace + "unknown diagnostic\n", link_map),
                                   (trace + "relative.a\n", link_map),
                                   (trace, "bad map"),
                                   (trace, link_map + "LOAD unreported.a\n"),
                                   (trace.replace(str(paths[0]), "C:/VC/Tools/MSVC/14/lib/crt2.o"), link_map),
                                   (trace.replace(str(paths[0]), "C:/rust/lib/self-contained/crt2.o"), link_map)]:
                with self.subTest(trace=broken), patch.dict(parse.__globals__, MINGW=mingw, ROOT=root):
                    with self.assertRaises(ValueError):
                        parse(broken.encode(), badmap.encode())
            for newline in ("\n", "\r\n"):
                raw_trace = trace.replace("\n", newline).encode()
                raw_map = link_map.replace("\n", newline).encode()
                with self.subTest(newline=newline), patch.dict(parse.__globals__, MINGW=mingw, ROOT=root):
                    result = parse(raw_trace, raw_map)
                    self.assertEqual(result["trace_raw_sha256"], hashlib.sha256(raw_trace).hexdigest())
                    self.assertEqual(result["trace"], self.module["normalize"](raw_trace.decode()))
                    for suffix in (" ", "\t", "garbage", "\rgarbage"):
                        with self.subTest(suffix=suffix), self.assertRaisesRegex(ValueError, "map LOAD records"):
                            parse(raw_trace, raw_map.replace(b"pei-x86-64)", b"pei-x86-64)" + suffix.encode()))

    def test_windows_paths_drive_spaces_and_archive_member_forms(self):
        parse = self.module.get("trace_path")
        self.assertIsNotNone(parse, "GNU trace paths need a strict parser")
        for line, expected in [(r"C:\mingw64\lib\crt2.o", (r"C:\mingw64\lib\crt2.o", None)),
                               ("C:/mingw64/path with spaces/lib.a", ("C:/mingw64/path with spaces/lib.a", None)),
                               ("(C:/mingw64/path with spaces/lib.a)part.o", ("C:/mingw64/path with spaces/lib.a", "part.o")),
                               ("C:/mingw64/lib.a(part.o)", ("C:/mingw64/lib.a", "part.o"))]:
            self.assertEqual(parse(line), expected)
        for line in ["unknown output", "C:relative.a", "relative.o", "-lmingw32", "(C:/lib.a)", "C:/lib.a(../escape.o)"]:
            with self.subTest(line=line), self.assertRaises(ValueError):
                parse(line)

    def test_pe_imports_allow_only_reviewed_windows_system_dlls(self):
        parse = self.module.get("pe_imports")
        self.assertIsNotNone(parse, "single-exe output needs native import verification")
        header = "app.exe: file format pei-x86-64\n"
        text = header + "\tDLL Name: KERNEL32.dll\n\tDLL Name: msvcrt.dll\n\tDLL Name: ucrtbase.dll\n"
        self.assertEqual(parse(text), ["kernel32.dll", "msvcrt.dll", "ucrtbase.dll"])
        for name in ["libgcc_s_seh-1.dll", "libwinpthread-1.dll", "libstdc++-6.dll", "VCRUNTIME140.dll", "MSVCP140.dll", "foreign.dll", "C:/kernel32.dll"]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                parse(text + f"DLL Name: {name}\n")
        for bad in ["", header, text.replace("pei-x86-64", "elf64-x86-64"), text + "DLL Name: ???\n",
                    text + "Entry d 000000001000 00000010 Delay Import Directory\n"]:
            with self.subTest(text=bad), self.assertRaises(ValueError):
                parse(bad)

    def test_pe_imports_accept_exact_preparation_inventory_and_normalize_case(self):
        parse = self.module["pe_imports"]
        previous = ("advapi32.dll", "bcrypt.dll", "crypt32.dll", "dbghelp.dll", "gdi32.dll", "iphlpapi.dll",
                    "kernel32.dll", "msvcrt.dll", "ntdll.dll", "ole32.dll", "oleaut32.dll", "psapi.dll",
                    "secur32.dll", "shell32.dll", "shlwapi.dll", "synchronization.dll", "ucrtbase.dll",
                    "user32.dll", "userenv.dll", "version.dll", "ws2_32.dll", "api-ms-win-core-synch-l1-2-0.dll")
        header = "app.exe: file format pei-x86-64\n"
        for name in APPROVED_PREPARATION_IMPORTS:
            with self.subTest(name=name):
                self.assertEqual(parse(header + f"\tDLL Name: {name.upper()}\r\n"), [name])
        inventory = previous + APPROVED_PREPARATION_IMPORTS
        self.assertEqual(parse(header + "".join(f"DLL Name: {name.upper()}\n" for name in inventory * 2)),
                         sorted(inventory))

    def test_preparation_import_exception_rejects_other_contracts_and_path_variants(self):
        header = "app.exe: file format pei-x86-64\n"
        approved = "".join(f"DLL Name: {name}\n" for name in APPROVED_PREPARATION_IMPORTS)
        rejected = ["api-ms-win-crt-time-l1-1-0.dll", "api-ms-win-crt-process-l1-1-0.dll",
                    "api-ms-win-crt-random-l1-1-0.dll", "api-ms-win-core-synch-l1-3-0.dll",
                    "api-ms-win-core-realtime-l1-1-0.dll", "bcryptprimitives-private.dll",
                    "api-ms-win-crt-private-l1-1-0-extra.dll",
                    "libgcc_s_seh-1.dll", "libwinpthread-1.dll", "MSVCP140.dll", "VCRUNTIME140.dll"]
        for name in APPROVED_PREPARATION_IMPORTS:
            rejected.extend(["C:/" + name, "../" + name, "private/" + name, "\"" + name + "\"", name + ".dll"])
            if "l1-1-0" in name:
                rejected.extend([name.replace("l1-1-0", "l1-1-1"), name.replace("l1-1-0", "l1-2-0")])
        for name in rejected:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "unreviewed PE DLL import"):
                self.module["pe_imports"](header + approved + f"DLL Name: {name}\n")
        with self.assertRaisesRegex(ValueError, "delay imports"):
            self.module["pe_imports"](header + approved + "Entry d 000000001000 00000010 Delay Import Directory\n")

    def test_collector_keeps_preparation_boundary_with_all_nine_imports(self):
        inventory = ["kernel32.dll", *APPROVED_PREPARATION_IMPORTS]
        text = "app.exe: file format pei-x86-64\n" + "".join(f"DLL Name: {name}\n" for name in inventory)
        evidence = self.collect(pe_text=text)
        self.assertTrue(evidence["preparation_only"])
        self.assertFalse(evidence["distribution_approved"])
        self.assertEqual(evidence["schema_version"], 2)
        self.assertEqual(evidence["pe_imports"], sorted(inventory))
        self.assertEqual(evidence["binary_sha256"], hashlib.sha256(b"fixture PE").hexdigest())
        self.assertEqual(evidence["pe_import_diagnostics"], text)
        for field in ("pe_import_diagnostics_sha256", "pe_import_captured_text_sha256"):
            self.assertEqual(evidence[field], hashlib.sha256(text.encode()).hexdigest())
        self.assertEqual(evidence["archive_members"], ["herdr-crossroads.exe", "herdr-crossroads.exe.sha256",
                                                     "LICENSE", "THIRD-PARTY-NOTICES.md"])
        calls = [args for args, options in evidence["calls"]]
        self.assertEqual(len([args for args in calls if args[:3] == ["cargo", "test", "--locked"]]), 10)
        for script in ("tests/test_windows_acceptance.py", "tests/test_windows_distribution.py"):
            self.assertTrue(any(script in args for args in calls))

    def test_pe_rejection_inventories_all_safe_unknown_names(self):
        text = ("app.exe: file format pei-x86-64\nDLL Name: KERNEL32.dll\n"
                "DLL Name: libwinpthread-1.dll\nDLL Name: libgcc_s_seh-1.dll\n")
        with self.assertRaisesRegex(ValueError, "unreviewed PE DLL import") as caught:
            self.module["pe_imports"](text)
        message = str(caught.exception)
        self.assertIn("libwinpthread-1.dll", message)
        self.assertIn("libgcc_s_seh-1.dll", message)
        diagnostic = json.loads(message.split("diagnostics=", 1)[1])
        self.assertEqual(diagnostic, {"names": ["libwinpthread-1.dll", "libgcc_s_seh-1.dll"],
                                     "unsupported_records": 2, "invalid_records": 0, "omitted_records": 0})

    def test_pe_rejection_inventories_nine_safe_unknown_names_without_omission(self):
        names = [f"unknown-{i}.dll" for i in range(9)]
        text = "app.exe: file format pei-x86-64\n" + "".join(f"DLL Name: {name}\n" for name in names)
        with self.assertRaisesRegex(ValueError, "unreviewed PE DLL import") as caught:
            self.module["pe_imports"](text)
        diagnostic = json.loads(str(caught.exception).split("diagnostics=", 1)[1])
        self.assertEqual(diagnostic, {"names": names, "unsupported_records": 9,
                                     "invalid_records": 0, "omitted_records": 0})

    def test_pe_rejection_bounds_names_and_counts_every_omitted_record(self):
        names = [str(i) + "x" * (124 - len(str(i))) + ".dll" for i in range(16)]
        self.assertTrue(all(len(name) == 128 for name in names))
        text = "app.exe: file format pei-x86-64\n" + "".join(f"DLL Name: {name}\n" for name in names)
        with self.assertRaises(ValueError) as caught:
            self.module["pe_imports"](text)
        payload = str(caught.exception).split("diagnostics=", 1)[1]
        self.assertLessEqual(len(payload), 2048)
        diagnostic = json.loads(payload)
        self.assertEqual(diagnostic, {"names": names[:12], "unsupported_records": 16,
                                     "invalid_records": 0, "omitted_records": 4})

    def test_pe_rejection_redacts_paths_controls_options_and_malformed_names(self):
        invalid = ["C:/private-person/secret.dll", r"C:\private-person\secret.dll",
                   "../private-person/secret.dll", "private-person/secret.dll", '"secret.dll"',
                   "secret name.dll", "secret\tname.dll", "secret\x1bname.dll", "secret\x00name.dll",
                   "secret\x7fname.dll", "sécret.dll", "-secret.dll", "--secret.dll", "???", "",
                   "x" * 125 + ".dll", "secret.dll ", "secret\x85name.dll"]
        for value in invalid:
            with self.subTest(value=value), self.assertRaises(ValueError) as caught:
                self.module["pe_imports"]("app.exe: file format pei-x86-64\nDLL Name: " + value + "\n")
            diagnostic = json.loads(str(caught.exception).split("diagnostics=", 1)[1])
            self.assertEqual(diagnostic, {"names": [], "unsupported_records": 1,
                                         "invalid_records": 1, "omitted_records": 0})
        with self.assertRaisesRegex(ValueError, "delay imports"):
            self.module["pe_imports"]("app.exe: file format pei-x86-64\nDLL Name: foreign.dll\n"
                                      "Entry d 000000001000 00000010 Delay Import Directory\n")

    def preflight_command(self, args, **kwargs):
        if "-dumpfullversion" in args:
            return "15.2.0\n"
        if "-dumpmachine" in args:
            return "x86_64-w64-mingw32\n"
        if "-print-prog-name=ld" in args:
            return "C:/mingw64/bin/ld.exe\n"
        if "-v" in args:
            return "Thread model: posix\n"
        if "-dM" in args:
            return "#define __MINGW64_VERSION_MAJOR 13\n#define __MINGW64_VERSION_MINOR 0\n#define __MINGW64_VERSION_BUGFIX 0\n#define _UCRT\n#define __SEH__ 1\n"
        name = "objdump" if args[0].endswith("objdump.exe") else "ld"
        return f"GNU {name} (GNU Binutils) 2.46.0.20260210\n"

    def test_preflight_accepts_only_exact_approved_binutils_build(self):
        preflight = self.module["toolchain_evidence"]
        for package in ("", "(GNU Binutils) "):
            for newline in ("\n", "\r\n"):
                calls = []
                def command(args, **kwargs):
                    calls.append(args)
                    if "--version" in args:
                        name = "objdump" if args[0].endswith("objdump.exe") else "ld"
                        return f"GNU {name} {package}2.46.0.20260210{newline}Copyright (C) 2026{newline}"
                    return self.preflight_command(args, **kwargs)
                with self.subTest(package=package, newline=newline), patch.dict(
                        preflight.__globals__, run=command), patch.object(Path, "read_bytes", return_value=b"tool"):
                    try:
                        evidence = preflight()
                    except ValueError as error:
                        self.fail(f"exact observed approved builder must pass: {error}")
                    self.assertEqual(len(calls), 7)
                    self.assertIsNone(evidence["installed_archive_identity"])
                    self.assertTrue(all("2.46.0.20260210" in text for text in evidence["binutils_versions"]))

    def test_preflight_rejection_identifies_each_failed_predicate_without_extra_commands(self):
        preflight = self.module["toolchain_evidence"]
        replacements = {"15.2.0": "15.1.0", "x86_64-w64-mingw32": "aarch64-w64-mingw32",
                        "C:/mingw64/bin/ld.exe": "C:/Users/private-person/ld.exe",
                        "posix": "win32", "2.46.0.20260210": "2.45", "MAJOR 13": "MAJOR 12",
                        "MINOR 0": "MINOR 1", "BUGFIX 0": "BUGFIX 2", "_UCRT": "_OTHER",
                        "__SEH__ 1": "__SEH__ 0"}
        calls = []
        def command(args, **kwargs):
            calls.append(args)
            text = self.preflight_command(args, **kwargs)
            for old, new in replacements.items():
                text = text.replace(old, new)
            return text
        with patch.dict(preflight.__globals__, run=command), patch.object(Path, "read_bytes") as read:
            with self.assertRaises(ValueError) as caught:
                preflight()
            read.assert_not_called()
        self.assertEqual(len(calls), 7)
        message = str(caught.exception)
        self.assertIn("; diagnostics=", message)
        self.assertIn("exact Binutils build 2.46.0.20260210", message)
        diagnostic = json.loads(message.split("; diagnostics=", 1)[1])
        expected_observed = {"gcc_version": ("15.2.0", "15.1.0"),
                             "machine": ("x86_64-w64-mingw32", "aarch64-w64-mingw32"),
                             "selected_linker_inside_approved_root": (True, False),
                             "thread_model": ("posix", "win32"),
                             "ld_version": ("2.46.0.20260210", ["2.45"]), "objdump_version": ("2.46.0.20260210", ["2.45"]),
                             "__MINGW64_VERSION_MAJOR": ("13", "12"),
                             "__MINGW64_VERSION_MINOR": ("0", "1"),
                             "__MINGW64_VERSION_BUGFIX": ("0", "2"),
                             "_UCRT": (True, False), "__SEH__": ("1", "0")}
        self.assertEqual(set(diagnostic["failing_checks"]), set(expected_observed))
        for name, (expected, observed) in expected_observed.items():
            self.assertEqual(diagnostic["expected"][name], expected)
            self.assertEqual(diagnostic["observed"][name], observed)
        self.assertNotIn("private-person", message)

    def test_preflight_diagnostics_mask_malformed_huge_and_untrusted_values(self):
        preflight = self.module["toolchain_evidence"]
        for unsafe in ("C:/Users/private-person/secret", "secret\t\x1b\x00", "secret" * 10000, "9" * 10000, ""):
            def command(args, **kwargs):
                if "-dM" in args:
                    return "#define __MINGW64_VERSION_MAJOR " + unsafe + "\n"
                return unsafe
            with self.subTest(value=unsafe[:20]), patch.dict(preflight.__globals__, run=command):
                with self.assertRaises(ValueError) as caught:
                    preflight()
                message = str(caught.exception)
                self.assertIn("; diagnostics=", message)
                diagnostic = json.loads(message.split("; diagnostics=", 1)[1])
                self.assertLess(len(message), 2048)
                self.assertNotIn("secret", message)
                self.assertNotIn("private-person", message)
                self.assertNotIn("\x1b", message)
                self.assertIn(diagnostic["observed"]["gcc_version"], ("absent", "unsupported"))

    def test_preflight_linker_hash_mismatch_has_distinct_safe_diagnostic(self):
        preflight = self.module["toolchain_evidence"]
        with patch.dict(preflight.__globals__, run=self.preflight_command), patch.object(
                Path, "read_bytes", side_effect=[b"selected", b"approved"]):
            with self.assertRaises(ValueError) as caught:
                preflight()
        self.assertIn("; diagnostics=", str(caught.exception))
        diagnostic = json.loads(str(caught.exception).split("; diagnostics=", 1)[1])
        self.assertEqual(diagnostic, {"expected": {"selected_linker_hash_matches": True},
                                      "observed": {"selected_linker_hash_matches": False},
                                      "failing_checks": ["selected_linker_hash_matches"]})

    def test_preflight_binutils_diagnostic_extracts_only_bounded_numeric_versions(self):
        preflight = self.module["toolchain_evidence"]
        def command(args, **kwargs):
            if "--version" in args:
                return "C:/Users/private-person/tool secret\x1b\n" + "2.45 " * 10000
            return self.preflight_command(args, **kwargs)
        with patch.dict(preflight.__globals__, run=command):
            with self.assertRaises(ValueError) as caught:
                preflight()
        message = str(caught.exception)
        self.assertIn("; diagnostics=", message)
        diagnostic = json.loads(message.split("; diagnostics=", 1)[1])
        self.assertEqual(diagnostic["failing_checks"], ["ld_version", "objdump_version"])
        for name in ("ld_version", "objdump_version"):
            self.assertEqual(diagnostic["observed"][name], "unsupported")
        self.assertLess(len(message), 2048)
        self.assertNotIn("private-person", message)
        self.assertNotIn("secret", message)

    def test_preflight_reports_date_versions_without_approving_them(self):
        preflight = self.module["toolchain_evidence"]
        for version in ("2.46", "2.46.0", "2.46.0.20260211", "2.46.0.20260209", "2.46.20260210",
                        "2.45", "2.45.1", "2.45.1.1", "2.45.50.20250727", "2.45.20250727"):
            for package in ("", "(GNU Binutils) "):
                for newline in ("\n", "\r\n"):
                    calls = []
                    def command(args, **kwargs):
                        calls.append(args)
                        if "--version" in args:
                            name = "objdump" if args[0].endswith("objdump.exe") else "ld"
                            return f"GNU {name} {package}{version}{newline}Copyright (C) 2.99{newline}"
                        return self.preflight_command(args, **kwargs)
                    with self.subTest(version=version, package=package, newline=newline), patch.dict(
                            preflight.__globals__, run=command), patch.object(Path, "read_bytes") as read:
                        with self.assertRaises(ValueError) as caught:
                            preflight()
                        read.assert_not_called()
                    diagnostic = json.loads(str(caught.exception).split("; diagnostics=", 1)[1])
                    self.assertEqual(diagnostic["failing_checks"], ["ld_version", "objdump_version"])
                    self.assertEqual(diagnostic["expected"]["ld_version"], "2.46.0.20260210")
                    for name in ("ld_version", "objdump_version"):
                        self.assertEqual(diagnostic["observed"][name], [version])
                    self.assertEqual(len(calls), 7)

    def test_preflight_binutils_versions_require_complete_first_line_tool_banner(self):
        preflight = self.module["toolchain_evidence"]
        for banner in ("Copyright (C) 2.45", "C:/Users/private-person/2.45/tool", "2.45",
                       "GNU other 2.45", "GNU {name} 2.45 secret", "GNU {name} 2.45\tsecret",
                       "GNU {name} 2.45\x1b", "GNU {name} 2.45\rsecret", "GNU {name} 2.45-private-person",
                       "GNU {name} (C:/Users/private-person/2.45) 2.45",
                       "GNU {name} " + "9" * 10000 + ".45", "GNU {name} 2.45.50.202507270",
                       "GNU {name} 2.45.50.99999", "GNU {name} 2.45.50.20251301",
                       "GNU {name} 2.45.50.20250727.1", "GNU {name} 2.45.50.20250727-1",
                       "private-person\nGNU {name} 2.45",
                       "GNU other 2.46.0.20260210", "GNU {name} 2.46.0.20260210 secret",
                       "GNU {name} 2.46.0.20260210\tsecret", "GNU {name} 2.46.0.20260210\x1b",
                       "GNU {name} " + "9" * 10000 + " 2.46.0.20260210",
                       "private-person\nGNU {name} 2.46.0.20260210"):
            def command(args, **kwargs):
                if "--version" in args:
                    name = "objdump" if args[0].endswith("objdump.exe") else "ld"
                    return banner.format(name=name) + "\nCopyright (C) 2.46 2.46.0.20260210\n"
                return self.preflight_command(args, **kwargs)
            with self.subTest(banner=banner[:80]), patch.dict(preflight.__globals__, run=command):
                with self.assertRaises(ValueError) as caught:
                    preflight()
            message = str(caught.exception)
            diagnostic = json.loads(message.split("; diagnostics=", 1)[1])
            for name in ("ld_version", "objdump_version"):
                self.assertEqual(diagnostic["observed"][name], "unsupported")
            self.assertLess(len(message), 2048)
            self.assertNotIn("private-person", message)
            self.assertNotIn("secret", message)

    def test_preflight_rejects_unknown_tool_versions_headers_and_driver(self):
        preflight = self.module.get("toolchain_evidence")
        self.assertIsNotNone(preflight, "strict toolchain preflight is required")
        command = self.preflight_command
        for newline in ("\n", "\r\n"):
            def native_command(args, **kwargs):
                return command(args, **kwargs).replace("\n", newline)
            with self.subTest(newline=newline), patch.dict(preflight.__globals__, run=native_command), patch.object(Path, "read_bytes", return_value=b"tool"):
                evidence = preflight()
                self.assertTrue(all(item["sha256"] == hashlib.sha256(b"tool").hexdigest() for item in evidence["executables"]))
                self.assertIsNone(evidence["installed_archive_identity"])
                self.assertEqual(evidence["header_macros"], native_command(["-dM"]))
                self.assertEqual(evidence["binutils_versions"], [native_command([str(tool), "--version"])
                                 for tool in (self.module["MINGW"] / "bin/ld.exe", self.module["MINGW"] / "bin/objdump.exe")])
            for macro in ("__MINGW64_VERSION_MAJOR 13", "__MINGW64_VERSION_MINOR 0", "__MINGW64_VERSION_BUGFIX 0", "__SEH__ 1"):
                for suffix in (" ", "\t", "garbage", "\rgarbage"):
                    def malformed(args, **kwargs):
                        return native_command(args, **kwargs).replace(macro + newline, macro + suffix + newline)
                    with self.subTest(newline=newline, macro=macro, suffix=suffix), patch.dict(preflight.__globals__, run=malformed), patch.object(Path, "read_bytes", return_value=b"tool"):
                        with self.assertRaisesRegex(ValueError, "unsupported GNU toolchain"):
                            preflight()
        for old, new in [("15.2.0", "15.1.0"), ("2.46", "2.45"), ("mingw32", "linux-gnu"),
                         ("MAJOR 13", "MAJOR 12"), ("_UCRT", "_OTHER"), ("posix", "win32"),
                         ("C:/mingw64/bin/ld.exe", "C:/other/ld.exe"), ("__SEH__", "__SJLJ__")]:
            def wrong(args, **kwargs):
                return command(args, **kwargs).replace(old, new)
            with self.subTest(old=old), patch.dict(preflight.__globals__, run=wrong), patch.object(Path, "read_bytes", return_value=b"tool"):
                with self.assertRaises(ValueError):
                    preflight()


if __name__ == "__main__":
    unittest.main()
