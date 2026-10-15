#!/usr/bin/env python3
"""Link once, retaining binary-bound evidence (not a license/readiness verdict)."""

import hashlib
import json
import os
from pathlib import Path
import re
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
    if not path.is_file():
        record["unavailable"] = True  # Rust's temporary link objects may already be gone.
        return record
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
    """Only recognize whole-line absolute file traces, not arbitrary map syntax."""
    paths = set()
    for line in stdout.splitlines():
        name = line.strip().split("(", 1)[0]
        if name.startswith("/") and not any(c in name for c in '\t\n"'):
            paths.add(name)
    return sorted(paths)


def collect(target, binary, stem):
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


if __name__ == "__main__":
    try:
        collect(sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]))
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Build provenance failed: {error}", file=sys.stderr)
        sys.exit(error.returncode if isinstance(error, subprocess.CalledProcessError) else 1)
