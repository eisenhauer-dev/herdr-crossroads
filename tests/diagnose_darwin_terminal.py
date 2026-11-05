"""Bounded observations on a disposable PTY, not a restoration workaround."""
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

if os.name == "posix":
    import pty
    import termios


def command(argv, **kwargs):
    return subprocess.run(argv, check=True, capture_output=True, text=True,
                          timeout=2, **kwargs).stdout.strip()


def emit(label, value):
    print(f"Darwin terminal diagnostic {label}={value!r}", flush=True)


def attributes(label, fd):
    value = termios.tcgetattr(fd)
    emit(label, value)
    return value


def transition(stty):
    # Exactly one trial, no slave reader or master input writer. Closing this
    # disposable PTY is cleanup, never compensation on an acceptance-test PTY.
    master, slave = pty.openpty()
    try:
        emit("trial", "no_reader; one raw-to-canonical transition; no input written")
        before = attributes("before_raw", slave)
        saved = command([stty, "-g"], stdin=slave)
        emit("saved_stty", saved)
        command([stty, "raw", "-echo", "min", "0", "time", "1"], stdin=slave)
        attributes("raw", slave)
        command([stty, saved], stdin=slave)
        restored = attributes("immediately_restored", slave)
        emit("differences", {name: (a, b) for name, a, b in zip(
            ("iflag", "oflag", "cflag", "lflag", "ispeed", "ospeed", "cc"), before, restored) if a != b})
    finally:
        os.close(master)
        os.close(slave)


def identity(label, executable):
    path = Path(executable)
    emit(label, {"selected": str(path), "realpath": str(path.resolve()),
                 "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    for name, argv in (("file", ["/usr/bin/file", "-b", str(path)]),
                       ("what", ["/usr/bin/what", str(path)])):
        try:
            emit(label + "_" + name, command(argv) or "no identification returned")
        except (OSError, subprocess.SubprocessError) as error:
            emit(label + "_" + name + "_unavailable", str(error))


def main():
    if sys.platform != "darwin":
        emit("SKIP", "Darwin-only diagnostic; acceptance gates are not skipped")
        return
    emit("python", sys.version)
    uname = platform.uname()
    emit("uname", {key: getattr(uname, key) for key in ("system", "release", "version", "machine")})
    emit("kernel_osrelease_osversion_version", command([
        "/usr/sbin/sysctl", "-n", "kern.osrelease", "kern.osversion", "kern.version"]))
    emit("sw_vers", command(["/usr/bin/sw_vers"]))
    selected = shutil.which("stty")
    if selected is None:
        raise RuntimeError("stty is not on PATH")
    identity("selected_stty", selected)
    identity("system_stty", "/bin/stty")
    emit("source_provenance", "unknown: binary hash/Mach-O identity is not an exact Apple source version or receipt")
    transition(selected)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Darwin terminal diagnostic failed: {error}", file=sys.stderr, flush=True)
        if isinstance(error, subprocess.CalledProcessError):
            print(error.stderr, file=sys.stderr, flush=True)
        sys.exit(1)
