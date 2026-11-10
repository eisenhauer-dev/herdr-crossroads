"""Native Windows preparation only: final link, fake-host gates, archive and evidence.

GNU input attribution and release approval remain separate gates.
This helper never publishes or installs tooling.
"""
import hashlib
import io
import json
import os
from pathlib import Path, PureWindowsPath
import platform
import re
import runpy
import shutil
import subprocess
import sys
import tarfile
import tempfile


ROOT = Path(__file__).absolute().parents[1]
TARGET = "x86_64-pc-windows-gnu"
MINGW = Path("C:/mingw64")
TOOLCHAIN = "1.99.0-x86_64-pc-windows-gnu"
EXPECTED_BINUTILS = "2.46.0.20260210"
process_helper = runpy.run_path(str(ROOT / "scripts/windows-process.py"))
supervise = process_helper["run"]
CAPTURE_LIMIT = process_helper["CAPTURE_LIMIT"]


def run(arguments, env=None, capture=False, timeout=None, capture_stderr=False, capture_limit=CAPTURE_LIMIT):
    result = supervise(arguments, cwd=ROOT, env=build_environment(env), capture=capture, timeout=timeout,
                       capture_limit=capture_limit)
    if capture and any(len(raw) >= capture_limit for raw in (result.stdout, result.stderr)):
        raise ValueError("required command evidence may have been truncated")
    return (result.stdout + (result.stderr if capture_stderr else b"")).decode("utf-8", "backslashreplace") if capture else ""


def build_environment(extra=None):
    return {**os.environ, **(extra or {}), "RUSTUP_TOOLCHAIN": TOOLCHAIN,
            "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER": str(MINGW / "bin/gcc.exe"),
            "CARGO_ENCODED_RUSTFLAGS": "\x1f".join(["-C", "link-self-contained=no", "-C", f"linker={MINGW / 'bin/gcc.exe'}"])}


def normalize(text):
    for path, label in sorted([(str(ROOT), "<repo>"), (str(Path.home()), "<home>"),
                               (os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")), "<cargo>"),
                               (os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup")), "<rustup>")],
                              key=lambda pair: len(pair[0]), reverse=True):
        pattern = r"[\\/]+".join(re.escape(part) for part in re.split(r"[\\/]", path))
        text = re.sub(pattern + r"(?=[\\/\s\"'():]|$)", lambda _: label, text, flags=re.I)
    return text


def trace_path(line):
    """GNU ld double trace: absolute files or archive/member records only."""
    member = None
    if line.startswith("("):
        match = re.fullmatch(r"\((.+)\)([^()]+)", line)
        if not match:
            raise ValueError("invalid GNU archive trace")
        line, member = match.groups()
    elif "(" in line or ")" in line:
        match = re.fullmatch(r"(.+)\(([^()]+)\)", line)
        if not match:
            raise ValueError("invalid GNU archive trace")
        line, member = match.groups()
    if (not (PureWindowsPath(line).is_absolute() or line.startswith("/"))
            or any(ord(c) < 32 or c in '\"()' for c in line)
            or not re.search(r"\.(?:o|obj|a|rlib)$", line, re.I)
            or (member is not None and not re.fullmatch(r"[\w.$@+-]+\.(?:o|obj)", member, re.ASCII))):
        raise ValueError("unsupported GNU trace path or member")
    return line, member


def path_key(path):
    return os.path.normpath(path.replace("\\", "/")).replace("\\", "/").lower()


def linker_trace(stdout, stderr):
    """Extract only the final binary's Rust 1.99 linker_messages JSON lint.

    Rust preserves UTF-8 GNU stdout in message, except for trim_end(). Do not
    parse rendered text, stderr, child notes, or reconstruct the removed bytes.
    """
    if any(len(raw) >= 1024 * 1024 for raw in (stdout, stderr)):
        raise ValueError("required GNU diagnostic capture may have been truncated")
    traces = []
    try:
        for line in stdout.decode("utf-8").splitlines():
            item = json.loads(line)
            if not isinstance(item, dict):
                raise ValueError
            target, message = item.get("target"), item.get("message")
            if (item.get("reason") != "compiler-message" or not isinstance(target, dict)
                    or not isinstance(message, dict)):
                continue
            code, text = message.get("code"), message.get("message")
            if (item.get("manifest_path") == str(ROOT / "Cargo.toml")
                    and target.get("name") == "herdr-crossroads" and target.get("kind") == ["bin"]
                    and target.get("src_path") == str(ROOT / "src/main.rs")
                    and isinstance(code, dict) and code.get("code") == "linker_messages"
                    and message.get("level") == "warning" and isinstance(text, str)
                    and text.startswith("linker stdout: ")):
                traces.append(text.removeprefix("linker stdout: ").encode("utf-8"))
    except (ValueError, UnicodeError):
        raise ValueError("invalid Cargo JSON diagnostic capture") from None
    if len(traces) != 1 or not traces[0]:
        raise ValueError("require exactly one final binary linker stdout diagnostic")
    return traces[0]


def map_load_rejection(records, number, value, reason):
    """Describe structure only: even known-root path tails and option values are private."""
    def describe(value):
        text = value.strip()
        path = text.lstrip('("\'')
        form = ["absolute" if PureWindowsPath(path).is_absolute() or path.startswith("/") else "relative"]
        extension = re.search(r"\.(o|obj|a|rlib|lib|dll|exe)(?:[\"')]|$)", text, re.I | re.ASCII)
        form.append("extension_" + extension[1].lower() if extension else "unknown_extension")
        for present, label in [(any(c in text for c in '\"\''), "quote"),
                               (any(c in text for c in "()"), "member"),
                               (any(ord(c) < 32 or ord(c) == 127 for c in value), "control")]:
            if present:
                form.append(label)

        def mask(match):
            extension = re.search(r"\.(?:o|obj|a|rlib|lib|dll|exe)$", match[0], re.I | re.ASCII)
            return "?" + (extension[0].lower() if extension else "")

        shape = re.sub(r"[^/\\:\"'() ]+", mask, text)
        shape = re.sub(r" +", " ", shape)
        return "|".join(form), shape[:120]

    form, shape = describe(value)
    diagnostic = {"line": number, "reason": reason, "form": form, "shape": shape,
                  "load_records": len(records), "forms": {}, "omitted_records": 0}
    for _, record in records:
        record_form, record_shape = describe(record)
        signature = record_form + ":" + record_shape[:32]
        if signature in diagnostic["forms"]:
            diagnostic["forms"][signature] += 1
        elif len(diagnostic["forms"]) < 16:
            diagnostic["forms"][signature] = 1
        else:
            diagnostic["omitted_records"] += 1
    payload = json.dumps(diagnostic, separators=(",", ":"))
    while len(payload) > 2048:
        _, count = diagnostic["forms"].popitem()
        diagnostic["omitted_records"] += count
        payload = json.dumps(diagnostic, separators=(",", ":"))
    return ValueError(f"unsupported GNU map LOAD at line {number}; diagnostics=" + payload)


def resolved_link_inputs(raw, raw_map):
    text = raw.decode("utf-8-sig")
    link_map = raw_map.decode("utf-8-sig")
    paths, members = {}, []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        try:
            path, member = trace_path(line.strip())
        except ValueError:
            # Unknown paths/options may contain personal data; disclose only location.
            raise ValueError(f"unsupported GNU trace at line {number}") from None
        key = path_key(path)
        if "/self-contained/" in key or "/msvc/" in key or key.endswith(".lib"):
            raise ValueError(f"non-external GNU input at trace line {number}")
        paths.setdefault(key, path)
        if member is not None:
            members.append({"path": normalize(path), "member": member, "trace_line": number})
    records, number = [], 1
    for chunk in link_map.splitlines(keepends=True):
        line = chunk.removesuffix("\r\n").removesuffix("\n")
        if line.startswith("LOAD "):
            records.append((number, line[5:]))
        number += chunk.count("\n")
    loads, synthetic = [], []
    for number, value in records:
        # GNU PE's in-memory filler BFD is map-only, never a filesystem input.
        if value == "dll stuff":
            if synthetic:
                raise map_load_rejection(records, number, value, "duplicate_synthetic")
            synthetic.append({"name": value, "kind": "gnu_pe_dll_filler", "map_line": number})
            continue
        try:
            loads.append(trace_path(value.strip()))
        except ValueError as error:
            reason = "archive_syntax" if str(error) == "invalid GNU archive trace" else "path_or_member"
            raise map_load_rejection(records, number, value, reason) from None
    if ("Linker script and memory map" not in link_map or not re.search(r"^OUTPUT\(.+ pei-x86-64\)\r?$", link_map, re.M)
            or not loads or any(m is not None for p, m in loads)
            or {path_key(p) for p, m in loads} != set(paths)):
        raise ValueError("GNU map LOAD records do not match final trace")
    # Rust 1.99 windows_gnu late_link_args names libmsvcrt.a even for a
    # UCRT-configured driver; never infer the CRT DLL from the archive basename.
    required = {"crt2.o", "crtbegin.o", "crtend.o", "libmingw32.a", "libmingwex.a", "libmsvcrt.a",
                "libgcc.a", "libgcc_eh.a", "libpthread.a"}
    external = path_key(str(MINGW)) + "/"
    found, files = set(), []
    for key, path in paths.items():
        name = key.rsplit("/", 1)[-1]
        if name in required:
            if not key.startswith(external):
                raise ValueError("GNU startup/runtime input is outside approved MinGW tree")
            found.add(name)
        record = {"path": normalize(path)}
        try:
            record.update(sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest(), availability="read_after_link")
        except FileNotFoundError:
            temporary = re.search(r"(?:^|/)(?:rustc[^/]+|\.rustc[^/]+|raw-dylibs)(?:/|$)", key) or name.endswith(".rcgu.o")
            if name in required or not temporary or not re.search(r"\.(?:o|obj|a)$", name):
                raise ValueError("required GNU input unavailable after link") from None
            record["availability"] = "missing_temporary_after_link"
        files.append(record)
    if found != required or not members:
        raise ValueError("missing GNU startup/runtime or archive member evidence")
    return {"source": "GNU ld --trace --trace and map LOAD records", "files": files,
            "archive_members": members, "synthetic_inputs": synthetic, "trace": normalize(text),
            "trace_raw_sha256": hashlib.sha256(raw).hexdigest(),
            "trace_normalized_sha256": hashlib.sha256(normalize(text).encode()).hexdigest(),
            "input_inventory_scope": "opened files; archive members separately reported, not complete symbol contributions",
            "symbol_contribution_completeness": "unknown"}


def pe_imports(text):
    if "file format pei-x86-64" not in text:
        raise ValueError("objdump did not identify PE x86_64")
    allowed = {"advapi32.dll", "bcrypt.dll", "crypt32.dll", "dbghelp.dll", "gdi32.dll", "iphlpapi.dll",
               "kernel32.dll", "msvcrt.dll", "ntdll.dll", "ole32.dll", "oleaut32.dll", "psapi.dll",
               "secur32.dll", "shell32.dll", "shlwapi.dll", "synchronization.dll", "ucrtbase.dll",
               "user32.dll", "userenv.dll", "version.dll", "ws2_32.dll", "api-ms-win-core-synch-l1-2-0.dll",
               # Exact observed imports approved for preparation only (2026-11-05), not compatibility certification.
               "bcryptprimitives.dll", "api-ms-win-crt-environment-l1-1-0.dll",
               "api-ms-win-crt-heap-l1-1-0.dll", "api-ms-win-crt-locale-l1-1-0.dll",
               "api-ms-win-crt-math-l1-1-0.dll", "api-ms-win-crt-private-l1-1-0.dll",
               "api-ms-win-crt-runtime-l1-1-0.dll", "api-ms-win-crt-stdio-l1-1-0.dll",
               "api-ms-win-crt-string-l1-1-0.dll"}
    names = []
    diagnostic = {"names": [], "unsupported_records": 0, "invalid_records": 0, "omitted_records": 0}
    for line in text.split("\n"):
        line = line.removesuffix("\r")
        if "Delay Import Directory" in line:
            match = re.fullmatch(r"\s*Entry d\s+0+\s+0+\s+Delay Import Directory\s*", line)
            if not match:
                raise ValueError("unreviewed PE delay imports")
        if "DLL Name:" in line:
            value = line.split("DLL Name:", 1)[1]
            name = value.strip().lower()
            if name not in allowed:
                diagnostic["unsupported_records"] += 1
                # Only bare ASCII filenames are public evidence, never path tails or raw input.
                value = value.removeprefix(" ")
                if len(value) > 128 or not re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_.-]*\.dll", value, re.I | re.ASCII):
                    diagnostic["invalid_records"] += 1
                elif len(diagnostic["names"]) < 12:
                    diagnostic["names"].append(value.lower())
                else:
                    diagnostic["omitted_records"] += 1
                continue
            names.append(name)
    if diagnostic["unsupported_records"]:
        raise ValueError("unreviewed PE DLL import; diagnostics=" + json.dumps(diagnostic, separators=(",", ":")))
    if not names:
        raise ValueError("missing PE DLL import evidence")
    return sorted(set(names))


def toolchain_evidence():
    gcc, ld, objdump = [MINGW / "bin" / (name + ".exe") for name in ("gcc", "ld", "objdump")]
    driver = run([str(gcc), "-dumpfullversion"], capture=True).strip()
    machine = run([str(gcc), "-dumpmachine"], capture=True).strip()
    selected = run([str(gcc), "-print-prog-name=ld"], capture=True).strip()
    config = run([str(gcc), "-v"], capture=True, capture_stderr=True)
    macros = run([str(gcc), "-dM", "-E", "-x", "c", "-include", "_mingw.h", "NUL"], capture=True)
    versions = [run([str(tool), "--version"], capture=True) for tool in (ld, objdump)]
    # Parse each tool's complete first line once for both predicates and safe diagnostics.
    date = r"[0-9]{4}(?:0[1-9]|1[0-2])(?:0[1-9]|[12][0-9]|3[01])"
    version = r"[0-9]{1,4}(?:\.[0-9]{1,4}){1,2}(?:\.(?:[0-9]{1,4}|" + date + r"))?"
    parsed_versions = {name: re.match(r"\AGNU " + name + r" (?:\(GNU Binutils\) )?(" + version + r")\r?(?:\n|\Z)", text, re.ASCII)
                       for name, text in zip(("ld", "objdump"), versions)}
    macro_versions = {"__MINGW64_VERSION_MAJOR": "13", "__MINGW64_VERSION_MINOR": "0",
                      "__MINGW64_VERSION_BUGFIX": "0", "__SEH__": "1"}
    inside = path_key(selected).startswith(path_key(str(MINGW)) + "/")
    checks = {"gcc_version": driver == "15.2.0", "machine": machine == "x86_64-w64-mingw32",
              "selected_linker_inside_approved_root": inside, "thread_model": "Thread model: posix" in config,
              **{name + "_version": bool(match and match[1] == EXPECTED_BINUTILS)
                 for name, match in parsed_versions.items()},
              **{name: bool(re.search(r"^#define " + name + r" " + value + r"\r?$", macros, re.M))
                 for name, value in macro_versions.items()},
              "_UCRT": bool(re.search(r"^#define _UCRT(?:\s|$)", macros, re.M))}
    if not all(checks.values()):
        # Only bounded grammar-checked values enter rejection logs, never raw captures or paths.
        def safe(value, grammar):
            if not value:
                return "absent"
            return value if len(value) <= 64 and re.fullmatch(grammar, value, re.ASCII) else "unsupported"

        numeric = r"[0-9]{1,4}(?:\.[0-9]{1,4}){0,3}"
        observed = {"gcc_version": safe(driver, numeric),
                    "machine": safe(machine, r"[a-z0-9_]+(?:-[a-z0-9_]+){2,4}"),
                    "selected_linker_inside_approved_root": inside, "_UCRT": checks["_UCRT"]}
        thread = re.search(r"^Thread model: ([^\r\n]*)\r?$", config, re.M)
        observed["thread_model"] = (thread[1] if thread and thread[1] in ("posix", "win32", "mcf")
                                    else "unsupported" if thread else "absent")
        for name in macro_versions:
            match = re.search(r"^#define " + name + r" ([^\r\n]*)\r?$", macros, re.M)
            observed[name] = safe(match[1] if match else "", numeric)
        for name, text in zip(("ld", "objdump"), versions):
            match = parsed_versions[name]
            observed[name + "_version"] = [match[1]] if match else ("unsupported" if text else "absent")
        expected = {"gcc_version": "15.2.0", "machine": "x86_64-w64-mingw32",
                    "selected_linker_inside_approved_root": True, "thread_model": "posix",
                    "ld_version": EXPECTED_BINUTILS, "objdump_version": EXPECTED_BINUTILS, **macro_versions, "_UCRT": True}
        diagnostic = {"expected": expected, "observed": observed,
                      "failing_checks": [name for name, passed in checks.items() if not passed]}
        raise ValueError(f"unsupported GNU toolchain: require GCC 15.2.0, exact Binutils build {EXPECTED_BINUTILS}, MinGW-w64 13.0.0 POSIX/SEH/UCRT"
                         + "; diagnostics=" + json.dumps(diagnostic, separators=(",", ":")))
    if hashlib.sha256(Path(selected).read_bytes()).digest() != hashlib.sha256(ld.read_bytes()).digest():
        raise ValueError('driver-selected linker differs from approved ld.exe; diagnostics='
                         '{"expected":{"selected_linker_hash_matches":true},'
                         '"observed":{"selected_linker_hash_matches":false},'
                         '"failing_checks":["selected_linker_hash_matches"]}')
    return {"gcc_version": driver, "machine": machine, "configuration": normalize(config),
            "header_macros": normalize(macros), "binutils_versions": versions,
            "installed_archive_identity": None,
            "executables": [{"path": str(p), "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for p in (gcc, ld, objdump, Path(selected))]}


def build():
    os.chdir(ROOT)
    installer = runpy.run_path(str(ROOT / "scripts/install-binary.py"))
    if platform.system() != "Windows" or installer["host_target"]() != TARGET:
        raise ValueError("Windows preparation requires native Windows 10+ x86_64")
    rust = run(["rustc", "-vV"], capture=True)
    cargo = run(["cargo", "--version"], capture=True)
    if not rust.startswith("rustc 1.99.0 ") or f"host: {TARGET}\n" not in rust or not cargo.startswith("cargo 1.99.0 "):
        raise ValueError("native Rust/Cargo 1.99.0 is required; install tooling separately")
    if sys.version_info < (3, 11):
        raise ValueError("Python 3.11+ is required")
    tools = toolchain_evidence()
    version = runpy.run_path(str(ROOT / "scripts/check-version.py"))["release_version"]()
    directory = ROOT / "target/builds"
    directory.mkdir(parents=True, exist_ok=True)
    stem = directory / f"herdr-crossroads-{version}-{TARGET}"
    outputs = [Path(str(stem) + suffix) for suffix in (".tar.gz", ".tar.gz.sha256", ".provenance.json", ".link.map")]
    binary = ROOT / "target" / TARGET / "release/herdr-crossroads.exe"
    for path in [*outputs, binary]:
        if path.is_symlink() or (path.exists() and not path.is_file()):
            raise ValueError(f"build output must be a regular non-symlink file: {path}")
        path.unlink(missing_ok=True)
    try:
        run([sys.executable, "scripts/third-party-notices.py", "--check", "--target", TARGET])
        # Force a fresh final link; never reuse Cargo's replayed map diagnostics.
        run(["cargo", "clean", "--release", "--target", TARGET, "--package", "herdr-crossroads"])
        argv = ["cargo", "rustc", "--release", "--locked", "--bin", "herdr-crossroads", "--target", TARGET,
                "--message-format=json", "--", "-W", "linker-messages",
                "-C", f"link-arg=-Wl,-Map,{outputs[3]}", "-C", "link-arg=-Wl,--trace,--trace"]
        linked = supervise(argv, cwd=ROOT, env=build_environment(), capture=True)
        data = binary.read_bytes()
        installer["verify_binary"](data, TARGET)
        raw_map = outputs[3].read_bytes()
        if not raw_map.strip():
            raise ValueError("GNU final link did not produce a nonempty map")
        raw_trace = linker_trace(linked.stdout, linked.stderr)
        inputs = resolved_link_inputs(raw_trace, raw_map)
        inputs["trace_capture"] = "Rust 1.99 linker_messages JSON message; UTF-8 stdout after trim_end(), not original linker bytes"
        outputs[3].write_bytes(normalize(raw_map.decode("utf-8", "surrogateescape")).encode("utf-8", "surrogateescape"))
        digest = hashlib.sha256(data).hexdigest()
        imports_text = run([str(MINGW / "bin/objdump.exe"), "-p", str(binary)], capture=True,
                           capture_limit=4 * CAPTURE_LIMIT)
        imports = pe_imports(imports_text)
        if hashlib.sha256(binary.read_bytes()).hexdigest() != digest:
            raise ValueError("final-linked binary changed during provenance collection")
        diagnostics = {}
        for name, raw in (("stdout", linked.stdout), ("stderr", linked.stderr)):
            text = normalize(raw.decode("utf-8", "backslashreplace"))
            diagnostics[name] = {"text": text, "raw_sha256": hashlib.sha256(raw).hexdigest(),
                                 "normalized_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest()}
        evidence = {"schema_version": 2, "preparation_only": True, "distribution_approved": False,
                    "target": TARGET, "version": version, "binary_sha256": digest,
                     "link_map_sha256": hashlib.sha256(outputs[3].read_bytes()).hexdigest(),
                     "link_map_raw_sha256": hashlib.sha256(raw_map).hexdigest(),
                     "toolchain": tools, "pe_imports": imports,
                     "pe_import_diagnostics": normalize(imports_text),
                     "pe_import_diagnostics_sha256": hashlib.sha256(normalize(imports_text).encode()).hexdigest(),
                     "pe_import_captured_text_sha256": hashlib.sha256(imports_text.encode()).hexdigest(),
                    "rust": rust, "cargo": cargo, "windows": platform.win32_ver(),
                    "command": [normalize(arg) for arg in argv], "source_commit": run(["git", "rev-parse", "HEAD"], capture=True).strip(),
                    "source_status": run(["git", "status", "--porcelain"], capture=True),
                    "link_output": normalize((linked.stdout + b"\n" + linked.stderr).decode("utf-8", "backslashreplace")),
                    "resolved_link_inputs": inputs,
                    "link_diagnostics": {**diagnostics, "scope": "supervisor-captured prefixes; raw hashes cover captured bytes only",
                                         "stream_completeness": "unknown"},
                     "open_gates": ["GNU linked-input attribution review", "native GNU CI", "independent review and release approval"]}
        evidence_bytes = (json.dumps(evidence, indent=2) + "\n").encode()
        outputs[2].write_bytes(evidence_bytes)
        raw_paths = {normalize(p): p for line in raw_trace.decode("utf-8-sig").splitlines() if line.strip()
                     for p, _ in [trace_path(line.strip())]}

        def verify_bound_inputs():
            if binary.read_bytes() != data:
                raise ValueError("final-linked binary changed during native testing")
            if hashlib.sha256(outputs[3].read_bytes()).hexdigest() != evidence["link_map_sha256"]:
                raise ValueError("link map changed during native testing")
            if outputs[2].read_bytes() != evidence_bytes:
                raise ValueError("provenance changed during native testing")
            if toolchain_evidence() != tools:
                raise ValueError("GNU tools changed during native testing")
            for item in inputs["files"]:
                if item["availability"] == "read_after_link":
                    if hashlib.sha256(Path(raw_paths[item["path"]]).read_bytes()).hexdigest() != item["sha256"]:
                        raise ValueError("GNU input changed during native testing")

        fake = ROOT / "target/windows-herdr.exe"
        run(["rustc", "--edition=2021", "--target", TARGET, "-C", f"linker={MINGW / 'bin/gcc.exe'}",
             "-C", "link-self-contained=no", "tests/fixtures/windows_herdr.rs", "-o", str(fake)])
        environment = {**os.environ, "CROSSROADS_WINDOWS_TEST_HERDR": str(fake),
                       "CROSSROADS_WINDOWS_BINARY": str(binary)}
        compiled = run(["cargo", "test", "--locked", "--test", "windows_console", "--no-run", "--message-format=json"], capture=True)
        tests = [json.loads(line) for line in compiled.splitlines() if line.startswith("{")]
        console_tests = [item["executable"] for item in tests if item.get("reason") == "compiler-artifact"
                         and item.get("target", {}).get("name") == "windows_console" and item.get("executable")]
        if len(console_tests) != 1:
            raise ValueError("could not select the native ConPTY console test executable")
        environment["CROSSROADS_WINDOWS_CONSOLE_TEST"] = console_tests[0]
        run(["cargo", "test", "--locked", "--lib", "test_windows", "--", "--test-threads=1"], env=environment, timeout=60)
        for group in ("test_cli", "test_input", "test_search", "test_selection::navigation", "test_mouse",
                      "test_actions::help_enter_restores_origin", "test_keymap::mouse_opt_out",
                      "test_keymap::enter_is_reserved", "test_keymap::lowercase_f"):
            run(["cargo", "test", "--locked", "--lib", group], env=environment, timeout=60)
        run(["cargo", "test", "--locked", "--test", "setup_dispatch"], env=environment, timeout=60)
        run([sys.executable, "-u", "-B", "tests/diagnose_windows_security.py"], env=environment, timeout=60)
        run([sys.executable, "-B", "tests/test_setup_keybind.py"], env=environment, timeout=60)
        run([sys.executable, "-B", "tests/test_windows_keybind.py"], env=environment, timeout=60)
        run([sys.executable, "-B", "tests/test_windows_acceptance.py"], env=environment, timeout=240)
        run([sys.executable, "-B", "tests/test_windows_distribution.py", "-v"], env=environment, timeout=60)
        verify_bound_inputs()
        members = {"herdr-crossroads.exe": data,
                   "herdr-crossroads.exe.sha256": f"{digest}  herdr-crossroads.exe\n".encode(),
                   **{name: (ROOT / name).read_bytes() for name in ("LICENSE", "THIRD-PARTY-NOTICES.md")}}
        with tarfile.open(outputs[0], "w:gz") as archive:
            for name, contents in members.items():
                info = tarfile.TarInfo(name)
                info.size, info.mode = len(contents), 0o755 if name.endswith(".exe") else 0o644
                archive.addfile(info, io.BytesIO(contents))
        archive_digest = hashlib.sha256(outputs[0].read_bytes()).hexdigest()
        checksum = f"{archive_digest}  {outputs[0].name}\n".encode()
        outputs[1].write_bytes(checksum)
        # Exercise the common checksum/archive/PE path and Windows installer on
        # the exact archive, then repeat native acceptance on the installed bytes.
        with tempfile.TemporaryDirectory(prefix="crossroads-package-") as temporary:
            checkout = Path(temporary) / "checkout with spaces"
            (checkout / "scripts").mkdir(parents=True)
            for name in ("install-binary.py", "check-version.py", "windows-files.py"):
                shutil.copy2(ROOT / "scripts" / name, checkout / "scripts" / name)
            for name in ("Cargo.toml", "Cargo.lock", "herdr-plugin.toml", "LICENSE", "THIRD-PARTY-NOTICES.md"):
                shutil.copy2(ROOT / name, checkout / name)
            environment["CROSSROADS_ASSET_DIR"] = str(directory)
            run([sys.executable, str(checkout / "scripts/install-binary.py")], env=environment)
            installed = checkout / "target/release/herdr-crossroads.exe"
            if installed.read_bytes() != data:
                raise ValueError("installed bytes differ from final-linked executable")
            environment["CROSSROADS_WINDOWS_BINARY"] = str(installed)
            run([sys.executable, "-B", "tests/test_windows_acceptance.py"], env=environment, timeout=240)
            if installed.read_bytes() != data:
                raise ValueError("installed binary changed during native testing")
        verify_bound_inputs()
        if hashlib.sha256(outputs[0].read_bytes()).hexdigest() != archive_digest or outputs[1].read_bytes() != checksum:
            raise ValueError("preparation archive changed during installed-byte testing")
        print("Windows preparation archive and evidence created; distribution approval and native support certification remain OPEN.")
    except BaseException:
        for output in outputs:
            output.unlink(missing_ok=True)
        raise


if __name__ == "__main__":
    try:
        if len(sys.argv) != 1:
            raise ValueError("build-windows takes no arguments")
        build()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Windows preparation failed: {error}", file=sys.stderr)
        sys.exit(1)
