# Coverage

## Policy and baseline

There is no numeric coverage minimum initially. Record measurements against the
baseline, explain decreases in review, and meaningfully test relevant changed
paths and error paths. A high aggregate percentage does not establish correctness.
The hard gates remain formatting, Clippy with `-D warnings`, Cargo tests, and all
four complete PTY harnesses against a freshly built optimized release binary;
see [Contributing](../CONTRIBUTING.md#test-layers-and-release-gates).

The native measurement on 2026-10-05 used Rust 1.99.0, its LLVM
23.1.1-rust-1.99.0-stable tools, and Linux x86_64. All 237 Cargo tests and the four
complete PTY harnesses passed with coverage instrumentation. Only repository
`src/` files contribute to these numerators and denominators, not tests,
dependencies, generated files, or the standard library:

| Metric | Covered / total | Coverage |
| --- | --- | --- |
| Lines | 3457 / 3643 | 94.89% |
| Functions | 303 / 315 | 96.19% |
| Regions | 5667 / 5977 | 94.81% |

Cargo-only and Cargo-plus-complete-PTY measurements had the same three aggregate
metrics. The inventory contained 27 Rust source files; 26 had coverage mappings.
`src/theme.rs` contains only module declarations and reexports, so it has no
executable mapping, rather than zero coverage. Branch and MC/DC coverage were not
emitted by this setup; they are **unavailable**, not 0%.

## Reproduce with native LLVM tools

Start from the repository root with the [developer prerequisites](../CONTRIBUTING.md#developer-setup),
including Linux, Python 3.11+, and a writable, pre-created `/tmp/opencode`.
Use the matching Rust/LLVM toolchain; 1.99.0 is the verification version, not an
MSRV or repository toolchain pin. If its LLVM tools component is missing, the
optional setup command is `rustup component add llvm-tools --toolchain 1.99.0`.
That setup command was not needed during verification. No `cargo-llvm-cov`,
additional Cargo dependency, or permanent helper script is required.

The following standard-library-only snippet creates a fresh `target/coverage.*`
directory with its own build and profiles. It stops on a failed command and
retains command arguments, stdout/stderr logs, Cargo artifact records, raw and
merged profiles, text reports, and source-only JSON exports. It needs no previous
ignored artifacts. Cargo may fetch locked dependencies on a fresh checkout; add
`--offline` to the Cargo test/build commands only when dependencies are cached.

```sh
python3 - <<'PY'
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

root = Path.cwd().resolve()
(root / "target").mkdir(exist_ok=True)
run = Path(tempfile.mkdtemp(prefix="coverage.", dir=root / "target"))
profiles = run / "profiles"
profiles.mkdir()
env = dict(os.environ, CARGO_TARGET_DIR=str(run / "build"),
           RUSTFLAGS="-C instrument-coverage",
           LLVM_PROFILE_FILE=str(profiles / "%m-%p.profraw"))
env.pop("CARGO_ENCODED_RUSTFLAGS", None)
commands = []
print(f"Artifacts: {run}", flush=True)

def execute(name, args, instrumented=False):
    args = [str(arg) for arg in args]
    commands.append({"name": name, "args": args})
    (run / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
    with (run / f"{name}.stdout").open("w") as out, \
         (run / f"{name}.stderr").open("w") as err:
        subprocess.run(args, env=env if instrumented else os.environ,
                       stdout=out, stderr=err, check=True)
    print(f"{name}: passed", flush=True)
    return (run / f"{name}.stdout").read_text()

cargo = ["cargo", "+1.99.0"]
version = execute("rustc-version", ["rustc", "+1.99.0", "-vV"])
host = next(line.split(": ", 1)[1] for line in version.splitlines()
            if line.startswith("host: "))
sysroot = execute("sysroot", ["rustc", "+1.99.0", "--print", "sysroot"]).strip()
llvm = Path(sysroot) / "lib" / "rustlib" / host / "bin"
execute("llvm-version", [llvm / "llvm-cov", "--version"])
metadata = json.loads(execute("metadata", cargo +
    ["metadata", "--no-deps", "--format-version=1", "--locked"]))
package = next(p["id"] for p in metadata["packages"]
               if Path(p["manifest_path"]).resolve() == root / "Cargo.toml")
test_log = execute("cargo-test", cargo +
    ["test", "--locked", "--message-format=json"], True)
cargo_profiles = sorted(profiles.glob("*.profraw"))
build_log = execute("cargo-build", cargo +
    ["build", "--locked", "--bins", "--message-format=json"], True)

def artifacts(log):
    for line in log.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue  # Cargo's JSON stream also includes plain test stdout.
        if (isinstance(item, dict) and item.get("reason") == "compiler-artifact"
                and item.get("package_id") == package and item.get("executable")):
            yield item

tests = sorted({item["executable"] for item in artifacts(test_log)
                if item["profile"]["test"]})
binary, = {item["executable"] for item in artifacts(build_log)
           if "bin" in item["target"]["kind"] and not item["profile"]["test"]}
assert len(tests) == 4, tests
(run / "objects.json").write_text(json.dumps({"binary": binary, "tests": tests}, indent=2))
sources = sorted(str(path.resolve()) for path in (root / "src").rglob("*.rs"))
selection = ["--include-filename-regex=^" + re.escape(str(root / "src") + "/"),
             "--sources", *sources]

def measure(name, raw):
    assert raw, "No profiles collected"
    merged = run / f"{name}.profdata"
    execute(f"{name}-merge", [llvm / "llvm-profdata", "merge", "-sparse",
                            *raw, "-o", merged])
    objects = [binary, *("--object=" + path for path in tests),
               "--instr-profile=" + str(merged), *selection]
    report = execute(f"{name}-report", [llvm / "llvm-cov", "report", *objects])
    execute(f"{name}-export", [llvm / "llvm-cov", "export", *objects])
    print(report, flush=True)

measure("cargo", cargo_profiles)
for harness in ("latency", "render", "keymaps", "hints"):
    execute(f"pty-{harness}", ["python3", f"tests/pty_{harness}.py", binary], True)
measure("combined", sorted(profiles.glob("*.profraw")))
PY
```

Inspect `cargo-test.stdout` for the test counts and each `pty-*.stdout` for the
complete harness result. The report `TOTAL` rows and export `data[].totals` give
the source-only aggregate metrics; exports retain per-file details for review.
The absolute `%m-%p.profraw` pattern distinguishes instrumented binaries and
processes, including subprocesses launched by Cargo tests and the PTY harnesses.
Both the explicit canonical source inventory and anchored filename filter keep
test/dependency mappings out of the report. Always use a fresh run directory;
never merge stale profiles or reuse objects from a different build.

## Limits

Forced termination can prevent profile flushing, so killed subprocesses may not
contribute their execution. Missing mapped lines and regions warrant inspection,
not an assumption that every uncovered path is untested. This fake-host Linux
measurement proves neither live Herdr integration nor non-Linux behavior.
Instrumented debug PTY latency is not a performance measurement and does not
replace the optimized-release latency gate. Coverage cannot replace assertions,
error-path tests, independent review, or the existing release checks.
