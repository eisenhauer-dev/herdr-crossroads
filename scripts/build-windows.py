"""Native Windows preparation only: final link, fake-host gates, archive and evidence.

MSVC/SDK attribution review, Windows 10 certification and real Herdr popup
acceptance remain separate gates. This helper never publishes or installs tooling.
"""
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import runpy
import shutil
import subprocess
import sys
import tarfile
import tempfile


ROOT = Path(__file__).absolute().parents[1]
TARGET = "x86_64-pc-windows-msvc"


def run(arguments, env=None, capture=False):
    result = subprocess.run(arguments, cwd=ROOT, env=env, check=True,
                            capture_output=capture, text=True, encoding="utf-8", errors="backslashreplace")
    return result.stdout if capture else ""


def normalize(text):
    for path, label in sorted([(str(ROOT), "<repo>"), (str(Path.home()), "<home>"),
                               (os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")), "<cargo>"),
                               (os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup")), "<rustup>")],
                              key=lambda pair: len(pair[0]), reverse=True):
        pattern = r"[\\/]+".join(re.escape(part) for part in re.split(r"[\\/]", path))
        text = re.sub(pattern + r"(?=[\\/\s\"'():]|$)", lambda _: label, text, flags=re.I)
    return text


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
                "--", "--print", "link-args", "-C", f"link-arg=/MAP:{outputs[3]}", "-C", "link-arg=/VERBOSE:LIB"]
        linked = subprocess.run(argv, cwd=ROOT, check=True, capture_output=True)
        data = binary.read_bytes()
        installer["verify_binary"](data, TARGET)
        raw_map = outputs[3].read_bytes()
        if not raw_map.strip():
            raise ValueError("MSVC final link did not produce a nonempty map")
        outputs[3].write_bytes(normalize(raw_map.decode("utf-8", "surrogateescape")).encode("utf-8", "surrogateescape"))
        digest = hashlib.sha256(data).hexdigest()
        evidence = {"schema_version": 1, "preparation_only": True, "distribution_approved": False,
                    "target": TARGET, "version": version, "binary_sha256": digest,
                    "link_map_sha256": hashlib.sha256(outputs[3].read_bytes()).hexdigest(),
                    "rust": rust, "cargo": cargo, "windows": platform.win32_ver(),
                    "command": [normalize(arg) for arg in argv], "source_commit": run(["git", "rev-parse", "HEAD"], capture=True).strip(),
                    "source_status": run(["git", "status", "--porcelain"], capture=True),
                    "link_output": normalize((linked.stdout + b"\n" + linked.stderr).decode("utf-8", "backslashreplace")),
                    "open_gates": ["MSVC/SDK linked-input attribution review", "Windows 10 certification", "real Herdr popup acceptance"]}
        outputs[2].write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        fake = ROOT / "target/windows-herdr.exe"
        run(["rustc", "--edition=2021", "tests/fixtures/windows_herdr.rs", "-o", str(fake)])
        environment = {**os.environ, "CROSSROADS_WINDOWS_TEST_HERDR": str(fake),
                       "CROSSROADS_WINDOWS_BINARY": str(binary)}
        compiled = run(["cargo", "test", "--locked", "--test", "windows_console", "--no-run", "--message-format=json"], capture=True)
        tests = [json.loads(line) for line in compiled.splitlines() if line.startswith("{")]
        console_tests = [item["executable"] for item in tests if item.get("reason") == "compiler-artifact"
                         and item.get("target", {}).get("name") == "windows_console" and item.get("executable")]
        if len(console_tests) != 1:
            raise ValueError("could not select the native ConPTY console test executable")
        environment["CROSSROADS_WINDOWS_CONSOLE_TEST"] = console_tests[0]
        run(["cargo", "test", "--locked", "--lib", "test_windows", "--", "--test-threads=1"], env=environment)
        for group in ("test_input", "test_search", "test_selection::navigation"):
            run(["cargo", "test", "--locked", "--lib", group], env=environment)
        run([sys.executable, "-B", "tests/test_windows_acceptance.py"], env=environment)
        run([sys.executable, "-B", "tests/test_windows_distribution.py"], env=environment)
        if hashlib.sha256(binary.read_bytes()).hexdigest() != digest:
            raise ValueError("final-linked binary changed during native testing")
        members = {"herdr-crossroads.exe": data,
                   "herdr-crossroads.exe.sha256": f"{digest}  herdr-crossroads.exe\n".encode(),
                   **{name: (ROOT / name).read_bytes() for name in ("LICENSE", "THIRD-PARTY-NOTICES.md")}}
        with tarfile.open(outputs[0], "w:gz") as archive:
            for name, contents in members.items():
                info = tarfile.TarInfo(name)
                info.size, info.mode = len(contents), 0o755 if name.endswith(".exe") else 0o644
                archive.addfile(info, io.BytesIO(contents))
        outputs[1].write_bytes(f"{hashlib.sha256(outputs[0].read_bytes()).hexdigest()}  {outputs[0].name}\n".encode())
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
            run([sys.executable, "-B", "tests/test_windows_acceptance.py"], env=environment)
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
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Windows preparation failed: {error}", file=sys.stderr)
        sys.exit(1)
