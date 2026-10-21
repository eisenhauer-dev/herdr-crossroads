"""Install a pinned GitHub Release (or explicit local asset); never build/activate."""

import gzip
import hashlib
import http.client
import io
import os
from pathlib import Path
import platform
import re
import runpy
import secrets
import shutil
import stat
import struct
import subprocess
import sys
import tarfile
import urllib.parse
import urllib.request
import zlib


# Immutable v0.1.0 source 490b0d74dc866b6ed83786f7aaa5657c35db5c51,
# and its full-text-equivalent Markdown (including original crate line endings).
LEGACY_NOTICE_SHA256 = "08b1ee55eea459afb4e0fe55c68ce91c2f1f815df9d1b9505ffecfdc9226b174"
LEGACY_MARKDOWN_SHA256 = "ba243d492753e7d6ce0832a14827d635c9066b354ed715456a492c46bde43e56"
# Released v0.2.0 Unix archives retain the 78-package Markdown inventory.
# Accept it only with this exact Windows-expanded preparation checkout inventory.
RELEASE_020_NOTICE_SHA256 = "ba243d492753e7d6ce0832a14827d635c9066b354ed715456a492c46bde43e56"
WINDOWS_PREP_NOTICE_SHA256 = "d18beb57a9837f22267622470a25bbac419646faf7b978888068b5a2377fffc4"


def require_https(url):
    parsed = urllib.parse.urlsplit(url)
    if (parsed.scheme != "https" or not parsed.hostname
            or parsed.username is not None or parsed.password is not None):
        raise ValueError("release downloads require uncredentialed HTTPS URLs")


class HTTPSRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        require_https(newurl)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def download(url, limit):
    require_https(url)
    curl = shutil.which("curl")
    if curl is not None:
        # Keep curl's native TLS verification; ignore curlrc and inherited proxies.
        command = [curl, "--disable", "--proto", "=https", "--proto-redir", "=https",
                   "--disallow-username-in-url", "--location", "--fail", "--silent", "--show-error",
                   "--noproxy", "*", "--max-time", "30", "--url", url]
        with subprocess.Popen(command, stdout=subprocess.PIPE) as child:
            try:
                # Bound unknown-length responses too, including on older curl.
                data = child.stdout.read(limit + 1)
                if len(data) > limit:
                    raise ValueError("release download exceeds size limit")
                if child.wait() != 0:
                    raise ValueError(f"curl release download failed (exit status {child.returncode})")
            finally:
                if child.poll() is None:
                    child.kill()
                child.wait()
        return data
    # No inherited proxy credentials or alternate proxy transport; use system TLS trust.
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), HTTPSRedirectHandler())
    with opener.open(url, timeout=30) as response:
        require_https(response.geturl())
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError("release download exceeds size limit")
    return data


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


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == "Linux" and machine == "x86_64" and platform.libc_ver()[0] == "glibc":
        return "x86_64-unknown-linux-gnu"
    if system == "Darwin" and machine in ("arm64", "aarch64", "x86_64"):
        version = platform.mac_ver()[0]
        if not re.fullmatch(r"[0-9]+\.[0-9]+(?:\.[0-9]+)?", version) or int(version.split(".")[0]) < 15:
            raise ValueError("macOS 15.0 or newer is required; cannot use an older or unknown OS version")
        # uname describes the running process under Rosetta: never probe hardware.
        architecture = "aarch64" if machine in ("arm64", "aarch64") else "x86_64"
        return architecture + "-apple-darwin"
    if system == "Windows" and machine in ("AMD64", "x86_64"):
        version = platform.win32_ver()[1]
        if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:\.[0-9]+)?", version) or int(version.split(".")[0]) < 10:
            raise ValueError("Windows 10 or newer is required; cannot use an older or unknown OS version")
        return "x86_64-pc-windows-msvc"
    raise ValueError("only Linux x86_64 glibc, macOS arm64/x86_64 and Windows x86_64 hosts are supported")


def verify_binary(binary, target):
    if target == "x86_64-pc-windows-msvc":
        if len(binary) < 64 or binary[:2] != b"MZ":
            raise ValueError("binary must be a PE32+ x86-64 console executable")
        offset = struct.unpack_from("<I", binary, 0x3c)[0]
        if offset < 64 or offset + 24 > len(binary) or binary[offset:offset + 4] != b"PE\0\0":
            raise ValueError("invalid PE header bounds or signature")
        machine, count, _, _, _, size, flags = struct.unpack_from("<HHIIIHH", binary, offset + 4)
        optional = offset + 24
        if (machine != 0x8664 or count == 0 or size < 112 or optional + size + count * 40 > len(binary)
                or not flags & 0x2 or flags & 0x2000
                or struct.unpack_from("<H", binary, optional)[0] != 0x20b
                or struct.unpack_from("<H", binary, optional + 68)[0] != 3):
            raise ValueError("binary must be a bounded PE32+ x86-64 console executable, not a DLL")
        return
    if target == "x86_64-unknown-linux-gnu":
        if (len(binary) < 64 or binary[:7] != b"\x7fELF\x02\x01\x01"
                or binary[16:18] not in [b"\x02\x00", b"\x03\x00"]
                or binary[18:24] != b"\x3e\x00\x01\x00\x00\x00"):
            raise ValueError("binary must have a little-endian ELF64 x86-64 executable header")
        return
    cpu = 0x0100000c if target == "aarch64-apple-darwin" else 0x01000007
    if (len(binary) < 32 or binary[:4] != b"\xcf\xfa\xed\xfe"
            or int.from_bytes(binary[4:8], "little") != cpu
            or int.from_bytes(binary[12:16], "little") != 2):
        raise ValueError("binary must be a thin little-endian Mach-O64 executable for " + target)
    count, size = struct.unpack_from("<2I", binary, 16)
    offset, end = 32, 32 + size
    macos = False
    if end > len(binary) or count > size // 8:
        raise ValueError("invalid Mach-O load command bounds")
    for _ in range(count):
        if offset + 8 > end:
            raise ValueError("truncated Mach-O load command")
        command, length = struct.unpack_from("<2I", binary, offset)
        if length < 8 or length % 8 or offset + length > end:
            raise ValueError("invalid Mach-O load command size")
        if command == 0x32:  # LC_BUILD_VERSION: require macOS, never iOS/simulator/Catalyst.
            if length < 24 or macos:
                raise ValueError("invalid Mach-O build version")
            product, minimum, _, tools = struct.unpack_from("<4I", binary, offset + 8)
            host = tuple(int(part) for part in platform.mac_ver()[0].split("."))
            host = host + (0,) * (3 - len(host))
            required = (minimum >> 16, (minimum >> 8) & 255, minimum & 255)
            if product != 1 or required > host or length != 24 + tools * 8:
                raise ValueError("Mach-O must target macOS at a supported OS version")
            macos = True
        elif command in (0x24, 0x25, 0x2f, 0x30):
            raise ValueError("Mach-O release requires an unambiguous macOS build version")
        offset += length
    if offset != end or not macos:
        raise ValueError("Mach-O release must contain a macOS build version")


def install():
    target = host_target()
    binary_name = "herdr-crossroads.exe" if target == "x86_64-pc-windows-msvc" else "herdr-crossroads"
    root = Path(__file__).absolute().parents[1]
    if Path(__file__).is_symlink():
        raise ValueError("installer must not be a symlink")
    version = runpy.run_path(str(root / "scripts/check-version.py"))["release_version"]()
    asset_dir = os.environ.get("CROSSROADS_ASSET_DIR")
    if asset_dir is not None and (not asset_dir or not Path(asset_dir).is_absolute()):
        raise ValueError("CROSSROADS_ASSET_DIR must name an absolute trusted local asset directory")
    archive_name = f"herdr-crossroads-{version}-{target}.tar.gz"
    if asset_dir is None:
        source = f"https://github.com/eisenhauer-dev/herdr-crossroads/releases/download/v{version}/"
        archive = download(source + archive_name, 64 * 1024 * 1024)
        checksum = download(source + archive_name + ".sha256", 1024)
    else:
        assets = Path(asset_dir)
        archive = (assets / archive_name).read_bytes()
        checksum = (assets / (archive_name + ".sha256")).read_bytes()
    verify_checksum(archive, checksum, archive_name)
    with gzip.GzipFile(fileobj=io.BytesIO(archive)) as compressed:
        expanded = compressed.read(128 * 1024 * 1024 + 1)
    if len(expanded) > 128 * 1024 * 1024:
        raise ValueError("expanded archive exceeds size limit")
    with tarfile.open(fileobj=io.BytesIO(expanded), mode="r:") as package:
        members = package.getmembers()
        notice_name = "THIRD-PARTY-NOTICES.md"
        names = {member.name for member in members}
        if version == "0.1.0" and names == {
                "herdr-crossroads", "herdr-crossroads.sha256", "LICENSE", "THIRD-PARTY-NOTICES.html"}:
            notice_name = "THIRD-PARTY-NOTICES.html"
        if (len(members) != 4 or {member.name for member in members}
                != {binary_name, binary_name + ".sha256", "LICENSE", notice_name}
                or any(member.type not in [tarfile.REGTYPE, tarfile.AREGTYPE]
                       or member.issparse() for member in members)):
            raise ValueError("archive must contain exactly the four regular release members")
        binary = package.extractfile(binary_name).read()
        checksum = package.extractfile(binary_name + ".sha256").read()
        notices = {}
        for name in ["LICENSE", notice_name]:
            data = package.extractfile(name).read()
            destination = "THIRD-PARTY-NOTICES.md" if name == notice_name else name
            trusted = (root / destination).read_bytes()
            if name == "THIRD-PARTY-NOTICES.html":
                # Pin BOTH representations: a later inventory cannot silently
                # stand in for the licenses accompanying the published binary.
                if (hashlib.sha256(data).hexdigest() != LEGACY_NOTICE_SHA256
                        or hashlib.sha256(trusted).hexdigest() != LEGACY_MARKDOWN_SHA256):
                    raise ValueError("legacy release notice does not match trusted Markdown inventory")
            elif data != trusted:
                if (version == "0.2.0" and target in (
                        "x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-apple-darwin")
                        and name == "THIRD-PARTY-NOTICES.md"
                        and hashlib.sha256(data).hexdigest() == RELEASE_020_NOTICE_SHA256
                        and hashlib.sha256(trusted).hexdigest() == WINDOWS_PREP_NOTICE_SHA256):
                    # Keep the licenses accompanying the released binary, not a
                    # later inventory that attributes additional Windows crates.
                    trusted = data
                else:
                    raise ValueError(f"release notice does not match trusted checkout: {name}")
            notices[destination] = trusted
    verify_checksum(binary, checksum, binary_name)
    verify_binary(binary, target)

    if target == "x86_64-pc-windows-msvc":
        runpy.run_path(str(root / "scripts/windows-files.py"))["install_files"](
            root, {**notices, binary_name: binary})
        return

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
    except (OSError, ValueError, KeyError, UnicodeError, EOFError,
            tarfile.TarError, http.client.HTTPException, zlib.error) as error:
        print(f"Binary installation failed: {error}", file=sys.stderr)
        sys.exit(1)
