#!/usr/bin/env python3
"""Link once, retaining binary-bound evidence (not a license/readiness verdict)."""

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys


CANDIDATES = (
    "Scrt1.o", "crt1.o", "crti.o", "crtn.o", "crtbeginS.o", "crtendS.o",
    "libc.so", "libc_nonshared.a", "libc.so.6", "ld-linux-x86-64.so.2",
    "libgcc.a", "libgcc_eh.a", "libgcc_s.so", "libgcc_s.so.1",
)
ENVIRONMENT = (
    "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN", "CARGO_NET_OFFLINE",
    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "CC", "CFLAGS", "LDFLAGS",
)


def run(argv, *, check=True):
    return subprocess.run(argv, text=True, capture_output=True, check=check,
                          env={**os.environ, "LC_ALL": "C"})


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normalize(text, root):
    prefixes = [(root, "<repo>"),
                (Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")), "<cargo>"),
                (Path(os.environ.get("RUSTUP_HOME", Path.home() / ".rustup")), "<rustup>"),
                (Path.home(), "<home>")]
    replacements = {}
    for path, label in prefixes:
        replacements[str(path.absolute())] = label
        replacements[str(path.resolve())] = label
    for prefix in sorted(replacements, key=len, reverse=True):
        text = re.sub(re.escape(prefix) + r"(?=/|[\s\"'():]|$)",
                      lambda match: replacements[prefix], text)
    return text


def file_record(path):
    """Keep both driver/trace spelling and resolved file; package ownership may differ."""
    path = Path(path)
    record = {"path": str(path), "resolved": str(path.resolve())}
    try:
        status = path.stat()
    except FileNotFoundError:
        record["unavailable"] = True  # Rust's temporary link objects may already be gone.
        return record
    if not stat.S_ISREG(status.st_mode):
        raise RuntimeError(f"Link input is not a regular file: {path}")
    record["sha256"] = sha256(path)
    packages = set()
    for name in dict.fromkeys([str(path), str(path.resolve())]):
        owners = run(["dpkg-query", "-S", name], check=False)
        for line in owners.stdout.splitlines():
            if ": " in line and not line.startswith("diversion "):
                packages.update(line.split(": ", 1)[0].split(", "))
    record["packages"] = []
    for package in sorted(packages):
        result = run(["dpkg-query", "-W", "-f=${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\n", package])
        for line in result.stdout.splitlines():
            name, version, source, source_version = line.split("\t")
            record["packages"].append(dict(name=name, version=version,
                                          source=source, source_version=source_version))
    return record


def traced_paths(stdout):
    """Index whitespace-free absolute paths before any archive member suffix."""
    paths = set()
    for line in stdout.splitlines():
        name = line.strip().split("(", 1)[0]
        if name.startswith("/") and not any(c.isspace() or c == '"' for c in name):
            paths.add(name)
    return sorted(paths)


def collect(target, binary, stem):
    if target.endswith("-apple-darwin"):
        return collect_darwin(target, binary, stem)
    root = Path.cwd().resolve()
    map_path = Path(str(stem) + ".link.map")
    json_path = Path(str(stem) + ".provenance.json")
    # Cargo can replay cached link diagnostics without recreating a deleted map.
    # Invalidate only this package; cached third-party dependencies remain intact.
    clean_argv = ["cargo", "clean", "--release", "--target", target,
                  "--package", "herdr-crossroads"]
    run(clean_argv)
    # Extra rustc arguments apply only to this binary, not dependencies/build scripts.
    argv = ["cargo", "rustc", "--release", "--locked", "--bin", "herdr-crossroads",
            "--target", target, "--", "--print", "link-args",
            "-C", f"link-arg=-Wl,-Map={map_path}",
            "-C", "link-arg=-Wl,--trace", "-C", "link-arg=-Wl,-v"]
    result = run(argv, check=False)
    # Keep diagnostics visible without publishing private checkout/home prefixes.
    print(normalize(result.stderr, root), end="", file=sys.stderr)
    if result.returncode:
        raise subprocess.CalledProcessError(result.returncode, argv)
    if not binary.is_file() or not os.access(binary, os.X_OK) or not map_path.is_file() or not map_path.stat().st_size:
        raise RuntimeError("link did not produce an executable and nonempty map")
    map_path.write_text(normalize(map_path.read_text(), root))
    candidates = []
    for name in CANDIDATES:
        resolved = run(["gcc", f"-print-file-name={name}"]).stdout.strip()
        candidates.append({"query": name, **file_record(resolved)})
    data = {
        "schema_version": 1,
        "target": target,
        "binary": {"path": str(binary), "sha256": sha256(binary)},
        "map": {"path": map_path.name, "sha256": sha256(map_path), "normalized": True},
        "build_argv": argv,
        "clean_argv": clean_argv,
        "environment": {"LC_ALL": "C", **{key: os.environ[key] for key in ENVIRONMENT if key in os.environ}},
        "versions": {"rustc": run(["rustc", "-vV"]).stdout,
                     "cargo": run(["cargo", "--version"]).stdout,
                     "gcc_candidate_driver": run(["gcc", "--version"]).stdout},
        "link_output": result.stdout,
        "link_diagnostics": result.stderr,
        "trace_files": [file_record(path) for path in traced_paths(result.stdout + "\n" + result.stderr)],
        "gcc_resolved_candidates": candidates,
        "limits": "Trace files are linker-opened inputs, not proof of incorporated code. "
                  "GCC candidates may differ from the selected driver/flags. Reconcile the raw map, "
                  "link argv and linker version output manually; no universal contribution parser or license verdict.",
    }
    json_path.write_text(normalize(json.dumps(data, indent=2) + "\n", root))


def collect_darwin(target, binary, stem):
    """Apple ld: sign during the one link, then inspect without modifying bytes."""
    root = Path.cwd().resolve()
    map_path = Path(str(stem) + ".link.map")
    json_path = Path(str(stem) + ".provenance.json")
    for path in [binary, map_path, json_path]:
        path.unlink(missing_ok=True)
    architectures = {"aarch64-apple-darwin": "arm64", "x86_64-apple-darwin": "x86_64"}
    if (target not in architectures or run(["uname", "-s"]).stdout.strip() != "Darwin"
            or run(["uname", "-m"]).stdout.strip() != architectures[target]):
        raise RuntimeError("Darwin provenance requires a matching native host")
    translated = run(["sysctl", "-in", "sysctl.proc_translated"], check=False)
    if translated.stdout.strip() == "1":
        raise RuntimeError("Rosetta is not a native release build")
    host_version = run(["sw_vers", "-productVersion"]).stdout.strip()
    if not re.fullmatch(r"\d+(?:\.\d+){1,2}", host_version) or int(host_version.split(".")[0]) < 15:
        raise RuntimeError("native macOS 15.0 or newer required")
    rust = run(["rustc", "-vV"]).stdout
    if (not re.search(r"^release: 1\.99\.0$", rust, re.M)
            or not re.search(r"^host: " + re.escape(target) + "$", rust, re.M)):
        raise RuntimeError("native Rust 1.99.0 toolchain required")
    if os.environ.get("MACOSX_DEPLOYMENT_TARGET") != "15.0":
        raise RuntimeError("MACOSX_DEPLOYMENT_TARGET must be 15.0")
    sdk = run(["xcrun", "--sdk", "macosx", "--show-sdk-path"]).stdout.strip()
    if not Path(sdk).is_dir():
        raise RuntimeError("macOS SDK is unavailable")
    sdk_root = Path(sdk).resolve()
    os.environ["SDKROOT"] = sdk
    clang = run(["xcrun", "--find", "clang"]).stdout.strip()
    clean = ["cargo", "clean", "--release", "--target", target, "--package", "herdr-crossroads"]
    argv = ["cargo", "rustc", "--release", "--locked", "--bin", "herdr-crossroads", "--target", target,
            "--", "--print", "link-args", "-C", f"linker={clang}", "-C", f"link-arg=-Wl,-map,{map_path}",
            "-C", "link-arg=-Wl,-t", "-C", "link-arg=-Wl,-v", "-C", "link-arg=-Wl,-adhoc_codesign"]
    try:
        run(clean)
        result = run(argv)
        print(normalize(result.stderr, root), end="", file=sys.stderr)
        if (binary.is_symlink() or not binary.is_file() or not os.access(binary, os.X_OK)
                or map_path.is_symlink() or not map_path.is_file() or not map_path.stat().st_size):
            raise RuntimeError("link did not produce an executable and nonempty map")
        binary_hash = sha256(binary)
        inspections = {}
        for name, command in {
            "architecture": ["lipo", "-archs", str(binary)],
            "headers": ["otool", "-hv", str(binary)],
            "load_commands": ["otool", "-l", str(binary)],
            "dylibs": ["otool", "-L", str(binary)],
            "signature": ["codesign", "--display", "--verbose=4", str(binary)],
            "signature_verification": ["codesign", "--verify", "--strict", "--verbose=4", str(binary)],
        }.items():
            output = run(command)
            inspections[name] = output.stdout + output.stderr
        if inspections["architecture"].strip() != architectures[target]:
            raise RuntimeError("Mach-O architecture differs from native target")
        if not re.search(r"\bEXECUTE\b", inspections["headers"]):
            raise RuntimeError("Mach-O is not an executable")
        loads = inspections["load_commands"]
        if not re.search(r"cmd LC_BUILD_VERSION\n(?:(?!Load command).)*?platform (?:1|macos|MACOS)\n\s*minos 15\.0(?:\.0)?\n", loads, re.S):
            raise RuntimeError("Mach-O deployment target must be macOS 15.0")
        if "Signature=adhoc" not in inspections["signature"]:
            raise RuntimeError("link-time ad-hoc signature required (not Developer ID)")
        dylibs = [line.strip().split(" (", 1)[0] for line in inspections["dylibs"].splitlines()[1:] if line.strip()]
        if not dylibs or any(path not in {"/usr/lib/libSystem.B.dylib", "/usr/lib/libiconv.2.dylib"}
                            for path in dylibs):
            raise RuntimeError(f"unreviewed Darwin dylib: licensing review required: {dylibs!r}")
        # Apple symbol names may contain opaque bytes; preserve them through normalization.
        raw_map = map_path.read_bytes().decode("utf-8", errors="surrogateescape")
        sysroot = Path(run(["rustc", "--print", "sysroot"]).stdout.strip())
        objects, sdk_stubs = darwin_objects(
            raw_map, root, sysroot, target, result.stdout + "\n" + result.stderr, sdk_root)
        if sha256(binary) != binary_hash:
            raise RuntimeError("binary changed during inspection")
        map_path.write_bytes(normalize(raw_map, root).encode("utf-8", errors="surrogateescape"))
        data = {
            "schema_version": 1, "target": target,
            "commit": run(["git", "rev-parse", "HEAD"]).stdout.strip(),
            "source_status": run(["git", "status", "--porcelain", "--untracked-files=no"]).stdout,
            "binary": {"path": str(binary), "sha256": binary_hash},
            "map": {"path": map_path.name, "sha256": sha256(map_path), "normalized": True},
            "build_argv": argv, "clean_argv": clean,
            "deployment_target": "15.0", "sdk": {"path": sdk, "resolved": str(sdk_root),
                "version": run(["xcrun", "--sdk", "macosx", "--show-sdk-version"]).stdout},
            "versions": {"rustc": rust, "cargo": run(["cargo", "--version"]).stdout,
                         "clang": run(["xcrun", "clang", "--version"]).stdout,
                         "os": run(["sw_vers"]).stdout,
                          "kernel": run(["uname", "-r"]).stdout},
            "inspections": inspections, "object_inputs": objects, "sdk_stub_inputs": sdk_stubs,
            "dylibs": dylibs,
            "link_output": result.stdout, "link_diagnostics": result.stderr,
            "limits": "Ad-hoc signed, not notarized or Developer ID signed. Map inputs are an inventory, "
                      "not a contribution/license verdict. Only Rust/project static objects and external "
                        "OS libSystem/libiconv are accepted. SDK text stubs are separately hashed dynamic "
                        "interfaces, not incorporated code; only UTF-8 TAPI v4 headers are supported, not a full YAML "
                        "validation. Unknown inputs require review. PTY gates follow this link.",
        }
        json_path.write_text(normalize(json.dumps(data, indent=2) + "\n", root))
    except BaseException:
        map_path.unlink(missing_ok=True)
        json_path.unlink(missing_ok=True)
        raise


def darwin_objects(text, root, sysroot, target, link_output, sdk=None):
    """Fail closed on Apple map changes or system/C static inputs needing notices."""
    if "# Object files:\n" not in text or "# Sections:\n" not in text:
        raise RuntimeError("unrecognized Apple link map: licensing review required")
    section = text.split("# Object files:\n", 1)[1].split("# Sections:\n", 1)[0]
    objects = []
    sdk_stubs = []
    deps = root / "target" / target / "release" / "deps"
    standard = sysroot / "lib" / "rustlib" / target / "lib"
    traces = {line.removeprefix("warning: linker stdout: ").strip()
              for line in link_output.splitlines()}
    sdk = Path(sdk).resolve() if sdk is not None else None
    stub_traces = {}
    for trace in sorted(traces):
        traced = Path(trace)
        if sdk is not None and traced.is_absolute() and traced.suffix == ".tbd":
            resolved = traced.resolve()
            if resolved.is_relative_to(sdk):
                stub_traces.setdefault(resolved, []).append(trace)
    for line in section.splitlines():
        match = re.fullmatch(r"\[\s*\d+\]\s+(.+)", line)
        if not match:
            raise RuntimeError("unrecognized Apple object entry: licensing review required")
        name = match[1]
        if name == "linker synthesized":
            objects.append(name)
            continue
        path = Path(name.split("(", 1)[0])
        if not path.is_absolute():
            path = root / path
        path = path.resolve()
        if (sdk is not None and Path(name).is_absolute() and Path(name).suffix == ".tbd"
                and path.suffix == ".tbd" and path.is_relative_to(sdk) and path in stub_traces):
            if not path.is_file():
                raise RuntimeError(f"unsupported SDK text stub (not a regular file): {name!r}")
            contents = path.read_bytes()
            try:
                contents.decode("utf-8", errors="strict")
            except UnicodeDecodeError:
                raise RuntimeError(f"unsupported SDK text stub (expected UTF-8 TAPI v4): {name!r}") from None
            # LLVM 20.1.8 TextStub.cpp: v4 uses this tag and required version key.
            # The successful native link validates the interface; this is a format signature, not a YAML parser.
            if (not re.match(rb"\A--- !tapi-tbd\ntbd-version:[ \t]+4[ \t]*\n", contents)
                    or any(byte < 32 and byte not in (9, 10, 13) for byte in contents)):
                raise RuntimeError(f"unsupported SDK text stub (expected UTF-8 TAPI v4): {name!r}")
            sdk_stubs.append({"path": name, "resolved": str(path),
                              "sha256": hashlib.sha256(contents).hexdigest(),
                              "trace_paths": stub_traces[path]})
            continue
        # rustc's generated Mach-O symbol references must also appear in this link's -t trace.
        generated_symbols = (name == str(path) and name in traces and path.name == "symbols.o"
                             and path.parent.parent == deps
                             and re.fullmatch(r"rustc[A-Za-z0-9]{6}", path.parent.name))
        if not ((path.parent == deps.resolve() and (path.suffix == ".rlib" or
                 (path.suffix == ".o" and path.name.startswith("herdr_crossroads-"))))
                or (path.parent == standard.resolve() and path.suffix == ".rlib")
                or generated_symbols):
            raise RuntimeError(f"unreviewed Darwin static input: licensing review required: {name!r} (resolved: {str(path)!r})")
        objects.append(name)
    if len(objects) < 2:
        raise RuntimeError("empty Apple object inventory")
    return objects, sdk_stubs


if __name__ == "__main__":
    try:
        collect(sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]))
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(normalize(f"Build provenance failed: {error}", Path.cwd().resolve()), file=sys.stderr)
        sys.exit(error.returncode if isinstance(error, subprocess.CalledProcessError) else 1)
