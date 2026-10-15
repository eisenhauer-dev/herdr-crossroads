"""Install one trusted local release asset; no downloads, builds, or activation."""

import hashlib
import io
import os
from pathlib import Path
import platform
import re
import runpy
import secrets
import stat
import sys
import tarfile


def verify_checksum(data, checksum, name):
    expected = hashlib.sha256(data).hexdigest().encode() + b"  " + name.encode() + b"\n"
    if not re.fullmatch(rb"[0-9a-f]{64}  " + re.escape(name.encode()) + rb"\n", checksum):
        raise ValueError(f"malformed checksum for {name}")
    if checksum != expected:
        raise ValueError(f"SHA256 mismatch for {name}")


def destination_directory(root):
    """Open every ancestor without following links; create only target/release."""
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    directory = os.open("/", flags)
    try:
        for part in root.parts[1:]:
            child = os.open(part, flags, dir_fd=directory)
            os.close(directory)
            directory = child
        for part in ["target", "release"]:
            try:
                os.mkdir(part, mode=0o755, dir_fd=directory)
            except FileExistsError:
                pass
            child = os.open(part, flags, dir_fd=directory)
            os.close(directory)
            directory = child
        return directory
    except BaseException:
        os.close(directory)
        raise


def install():
    if (platform.system() != "Linux" or platform.machine() != "x86_64"
            or platform.libc_ver()[0] != "glibc"):
        raise ValueError("only Linux x86_64 glibc hosts are supported")
    root = Path(__file__).absolute().parents[1]
    if Path(__file__).is_symlink():
        raise ValueError("installer must not be a symlink")
    version = runpy.run_path(str(root / "scripts/check-version.py"))["release_version"]()
    asset_dir = os.environ.get("CROSSROADS_ASSET_DIR")
    if not asset_dir or not Path(asset_dir).is_absolute():
        raise ValueError("CROSSROADS_ASSET_DIR must name an absolute trusted local asset directory")
    archive_name = f"herdr-crossroads-{version}-x86_64-unknown-linux-gnu.tar.gz"
    assets = Path(asset_dir)
    archive = (assets / archive_name).read_bytes()
    verify_checksum(archive, (assets / (archive_name + ".sha256")).read_bytes(), archive_name)
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as package:
        members = package.getmembers()
        if (len(members) != 4 or {member.name for member in members}
                != {"herdr-crossroads", "herdr-crossroads.sha256", "LICENSE", "THIRD-PARTY-NOTICES.html"}
                or any(member.type not in [tarfile.REGTYPE, tarfile.AREGTYPE]
                       or member.issparse() for member in members)):
            raise ValueError("archive must contain exactly the four regular release members")
        binary = package.extractfile("herdr-crossroads").read()
        checksum = package.extractfile("herdr-crossroads.sha256").read()
        notices = {}
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.html"]:
            notices[name] = package.extractfile(name).read()
            if notices[name] != (root / name).read_bytes():
                raise ValueError(f"release notice does not match trusted checkout: {name}")
    verify_checksum(binary, checksum, "herdr-crossroads")
    if (len(binary) < 64 or binary[:7] != b"\x7fELF\x02\x01\x01"
            or binary[16:18] not in [b"\x02\x00", b"\x03\x00"]
            or binary[18:24] != b"\x3e\x00\x01\x00\x00\x00"):
        raise ValueError("binary must have a little-endian ELF64 x86-64 executable header")

    directory = destination_directory(root)
    staged = []
    try:
        files = {**notices, "herdr-crossroads": binary}
        for name in files:
            try:
                existing = os.stat(name, dir_fd=directory, follow_symlinks=False)
            except FileNotFoundError:
                existing = None
            if existing is not None and not stat.S_ISREG(existing.st_mode):
                raise ValueError("runtime destination must be a regular file, not a symlink")
        for name, data in files.items():
            staging = ".herdr-crossroads-" + secrets.token_hex(12)
            descriptor = os.open(staging, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                                 0o600, dir_fd=directory)
            staged.append((staging, name))
            with os.fdopen(descriptor, "wb") as output:
                output.write(data)
                output.flush()
                os.fchmod(output.fileno(), 0o755 if name == "herdr-crossroads" else 0o644)
                os.fsync(output.fileno())
        # Notices must be retained before the runtime is replaced.
        while staged:
            staging, name = staged[0]
            os.replace(staging, name, src_dir_fd=directory, dst_dir_fd=directory)
            staged.pop(0)
    finally:
        for staging, _ in staged:
            os.unlink(staging, dir_fd=directory)
        os.close(directory)
    print(f"Installed {archive_name} at target/release/herdr-crossroads")


if __name__ == "__main__":
    try:
        install()
    except (OSError, ValueError, KeyError, UnicodeError, tarfile.TarError) as error:
        print(f"Local binary installation failed: {error}", file=sys.stderr)
        sys.exit(1)
