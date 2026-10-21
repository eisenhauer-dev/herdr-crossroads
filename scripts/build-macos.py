#!/usr/bin/env python3
"""Native macOS release gates and stdlib-only packaging; no post-test mutation."""
import hashlib
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def run(argv, *, timeout=None):
    if timeout is None:
        return subprocess.run(argv, cwd=ROOT, check=True, text=True, capture_output=True)
    # PTY children inherit this session/group; kill the whole gate on interruption.
    process = subprocess.Popen(argv, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except BaseException as error:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        stdout, stderr = process.communicate()
        if isinstance(error, subprocess.TimeoutExpired):
            raise RuntimeError(f"PTY command timed out after {timeout}s: {' '.join(argv)}\n"
                               f"{stdout}\n{stderr}") from error
        raise
    result = subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)
    result.check_returncode()
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def regular(path):
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"not a regular non-symlink file: {path.name}")
    return path.read_bytes()


def build():
    os.environ.update(CARGO_TARGET_DIR=str(ROOT / "target"), RUSTUP_TOOLCHAIN="1.99.0",
                      MACOSX_DEPLOYMENT_TARGET="15.0", LC_ALL="C")
    arch = run(["uname", "-m"]).stdout.strip()
    targets = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}
    if arch not in targets or run(["uname", "-s"]).stdout.strip() != "Darwin":
        raise RuntimeError("only native macOS arm64/x86_64 builds are supported")
    target = targets[arch]
    version = run(["python3", "scripts/check-version.py"]).stdout.strip()
    output = ROOT / "target/builds"
    output.mkdir(parents=True, exist_ok=True)
    stem = output / f"herdr-crossroads-{version}-{target}"
    archive = Path(str(stem) + ".tar.gz")
    checksum = Path(str(archive) + ".sha256")
    artifacts = [archive, checksum, Path(str(stem) + ".provenance.json"),
                 Path(str(stem) + ".link.map"), Path(str(archive) + ".tmp"),
                 Path(str(checksum) + ".tmp")]
    binary = ROOT / f"target/{target}/release/herdr-crossroads"
    try:
        for path in artifacts:
            path.unlink(missing_ok=True)
        # Reject hidden compiler/linker overrides instead of trusting their licensing.
        for key in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
                    "RUSTC_WORKSPACE_WRAPPER", "LDFLAGS", "CFLAGS", "CC",
                    "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER",
                    "CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER"]:
            if os.environ.get(key):
                raise RuntimeError(f"release build does not accept {key}")
        run(["python3", "scripts/third-party-notices.py", "--check", "--target", target])
        binary.unlink(missing_ok=True)
        Path(str(binary) + ".sha256").unlink(missing_ok=True)
        run(["python3", "scripts/build-provenance.py", target, str(binary), str(stem)])
        data = regular(binary)
        if not os.access(binary, os.X_OK):
            raise RuntimeError("linked binary is not executable")
        evidence = json.loads(regular(Path(str(stem) + ".provenance.json")))
        if digest(data) != evidence["binary"]["sha256"]:
            raise RuntimeError("binary changed since link evidence")
        for gate in ["latency", "render", "keymaps", "hints"]:
            result = run(["python3", f"tests/pty_{gate}.py", str(binary)], timeout=300)
            print(result.stdout, end="")
            print(result.stderr, end="", file=sys.stderr)
            if regular(binary) != data:
                raise RuntimeError("binary changed during PTY gates")
        files = [("herdr-crossroads", data),
                 ("herdr-crossroads.sha256", (digest(data) + "  herdr-crossroads\n").encode()),
                 ("LICENSE", regular(ROOT / "LICENSE")),
                 ("THIRD-PARTY-NOTICES.md", regular(ROOT / "THIRD-PARTY-NOTICES.md"))]
        with tarfile.open(str(archive) + ".tmp", "w:gz", format=tarfile.USTAR_FORMAT) as package:
            for name, content in files:
                member = tarfile.TarInfo(name)
                member.mode = 0o755 if name == "herdr-crossroads" else 0o644
                member.size = len(content)
                package.addfile(member, io.BytesIO(content))
        Path(str(archive) + ".tmp").replace(archive)
        Path(str(checksum) + ".tmp").write_text(digest(archive.read_bytes()) + f"  {archive.name}\n")
        Path(str(checksum) + ".tmp").replace(checksum)
    except BaseException:
        for path in artifacts:
            path.unlink(missing_ok=True)
        raise
    print(f"Built and passed all four full PTYs: target/builds/{archive.name}")
    return archive


if __name__ == "__main__":
    for signum in [signal.SIGINT, signal.SIGTERM]:
        signal.signal(signum, lambda number, frame: sys.exit(128 + number))
    try:
        build()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        # Child diagnostics have already been normalized by their collectors.
        if isinstance(error, subprocess.CalledProcessError):
            print(error.stderr or "release command failed", file=sys.stderr)
        else:
            print(f"macOS packaging failed: {error}", file=sys.stderr)
        sys.exit(error.returncode if isinstance(error, subprocess.CalledProcessError) else 1)
