"""Windows installation/config writes: pin directories; reject reparse points."""

from contextlib import contextmanager, ExitStack
import ctypes
from ctypes import wintypes
import os
from pathlib import Path
import secrets
import stat


KERNEL = ctypes.WinDLL("kernel32", use_last_error=True)
KERNEL.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                              ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
KERNEL.CreateFileW.restype = wintypes.HANDLE
KERNEL.CloseHandle.argtypes = [wintypes.HANDLE]
KERNEL.CloseHandle.restype = wintypes.BOOL
KERNEL.GetFileInformationByHandleEx.argtypes = [wintypes.HANDLE, ctypes.c_int,
                                              ctypes.c_void_p, wintypes.DWORD]
KERNEL.GetFileInformationByHandleEx.restype = wintypes.BOOL
KERNEL.ReplaceFileW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.LPCWSTR,
                               wintypes.DWORD, ctypes.c_void_p, ctypes.c_void_p]
KERNEL.ReplaceFileW.restype = wintypes.BOOL
KERNEL.GetVolumeInformationW.argtypes = [wintypes.LPCWSTR, wintypes.LPWSTR, wintypes.DWORD,
                                       ctypes.c_void_p, ctypes.c_void_p,
                                       ctypes.POINTER(wintypes.DWORD), wintypes.LPWSTR, wintypes.DWORD]
KERNEL.GetVolumeInformationW.restype = wintypes.BOOL
SECURITY = ctypes.WinDLL("advapi32", use_last_error=True)
SECURITY.GetKernelObjectSecurity.argtypes = [wintypes.HANDLE, wintypes.DWORD, ctypes.c_void_p,
                                           wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
SECURITY.GetKernelObjectSecurity.restype = wintypes.BOOL
SECURITY.SetSecurityDescriptorControl.argtypes = [ctypes.c_void_p, wintypes.WORD, wintypes.WORD]
SECURITY.SetSecurityDescriptorControl.restype = wintypes.BOOL
SECURITY.GetSecurityDescriptorControl.argtypes = [ctypes.c_void_p, ctypes.POINTER(wintypes.WORD),
                                                ctypes.POINTER(wintypes.DWORD)]
SECURITY.GetSecurityDescriptorControl.restype = wintypes.BOOL
SECURITY.SetFileSecurityW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, ctypes.c_void_p]
SECURITY.SetFileSecurityW.restype = wintypes.BOOL


class AttributeTag(ctypes.Structure):
    _fields_ = [("attributes", wintypes.DWORD), ("tag", wintypes.DWORD)]


class SecurityAttributes(ctypes.Structure):
    _fields_ = [("length", wintypes.DWORD), ("descriptor", ctypes.c_void_p),
                ("inherit", wintypes.BOOL)]


@contextmanager
def pinned_directory(path):
    # FILE_LIST_DIRECTORY|FILE_READ_ATTRIBUTES, share READ|WRITE but NOT DELETE,
    # OPEN_EXISTING, BACKUP_SEMANTICS|OPEN_REPARSE_POINT. Attribute-only access
    # does not enforce sharing restrictions. Holding each ancestor prevents a
    # concurrent directory/junction swap while path-based operations run below.
    handle = KERNEL.CreateFileW(str(path), 0x81, 3, None, 3, 0x02200000, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        info = AttributeTag()
        if not KERNEL.GetFileInformationByHandleEx(handle, 9, ctypes.byref(info), ctypes.sizeof(info)):
            raise ctypes.WinError(ctypes.get_last_error())
        if not info.attributes & 0x10 or info.attributes & 0x400:
            raise ValueError(f"installation ancestor must be a non-reparse directory: {path}")
        yield path
    finally:
        KERNEL.CloseHandle(handle)


@contextmanager
def pinned_destination(root):
    root = Path(root).absolute()
    if len(root.anchor) != 3 or root.anchor[1:] != ":\\" or any(p in (".", "..") for p in root.parts):
        raise ValueError("Windows installation requires a local absolute drive path")
    with ExitStack() as stack:
        path = Path(root.anchor)
        stack.enter_context(pinned_directory(path))
        for part in root.parts[1:]:
            path /= part
            stack.enter_context(pinned_directory(path))
        for part in ("target", "release"):
            path /= part
            try:
                path.mkdir()
            except FileExistsError:
                pass
            stack.enter_context(pinned_directory(path))
        yield path


def install_files(root, files):
    with pinned_destination(root) as destination:
        staged = []
        try:
            for name, data in files.items():
                if name not in {"herdr-crossroads.exe", "LICENSE", "THIRD-PARTY-NOTICES.md"}:
                    raise ValueError("unexpected Windows installation member")
                target = destination / name
                try:
                    info = target.lstat()
                except FileNotFoundError:
                    pass
                else:
                    if not stat.S_ISREG(info.st_mode) or info.st_file_attributes & 0x400:
                        raise ValueError(f"installation target must be a regular non-reparse file: {name}")
                temporary = destination / (".crossroads-" + secrets.token_hex(16))
                fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_BINARY, 0o600)
                staged.append(temporary)
                with os.fdopen(fd, "wb") as output:
                    output.write(data)
                    output.flush()
                    os.fsync(output.fileno())
                # A running executable is locked on Windows: replacement fails
                # without truncating the existing file. Close the popup and retry.
                os.replace(temporary, target)
                staged.remove(temporary)
        finally:
            for temporary in staged:
                temporary.unlink(missing_ok=True)


def config_identity(info, *, creation_time=False):
    if info is None:
        return None
    # Access time may change during reads, not during an edit.
    timestamp = (getattr(info, "st_birthtime_ns", info.st_ctime_ns)
                 if creation_time else info.st_ctime_ns)
    return (info.st_dev, info.st_ino, info.st_mode, info.st_nlink,
            info.st_size, info.st_mtime_ns, timestamp)


def temp_directory():
    # Match Rust std::env::temp_dir, not Python's TMPDIR-first search/cache.
    function = getattr(KERNEL, "GetTempPath2W", KERNEL.GetTempPathW)
    function.argtypes = [wintypes.DWORD, wintypes.LPWSTR]
    function.restype = wintypes.DWORD
    buffer = ctypes.create_unicode_buffer(32768)
    length = function(len(buffer), buffer)
    if not length:
        raise ctypes.WinError(ctypes.get_last_error())
    if length >= len(buffer):
        raise ValueError("Windows temporary directory exceeds the native path limit")
    return Path(buffer.value)


def config_security(handle):
    needed = wintypes.DWORD()
    SECURITY.GetKernelObjectSecurity(handle, 7, None, 0, ctypes.byref(needed))
    if not needed.value:
        raise ctypes.WinError(ctypes.get_last_error())
    security = ctypes.create_string_buffer(needed.value)
    if not SECURITY.GetKernelObjectSecurity(handle, 7, security, needed, ctypes.byref(needed)):
        raise ctypes.WinError(ctypes.get_last_error())
    return security


def config_snapshot(path):
    import msvcrt

    try:
        before = path.lstat()
    except FileNotFoundError:
        return None, b"", None
    if (not stat.S_ISREG(before.st_mode) or before.st_file_attributes & (0x400 | 1)
            or before.st_nlink != 1):
        raise ValueError("config must be a writable single-link regular non-reparse file")
    # GENERIC_READ, share READ only, OPEN_EXISTING, OPEN_REPARSE_POINT:
    # refuse existing writers/delete handles and exclude them while reading.
    handle = KERNEL.CreateFileW(str(path), 0x80000000, 1, None, 3, 0x00200000, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        tag = AttributeTag()
        if not KERNEL.GetFileInformationByHandleEx(handle, 9, ctypes.byref(tag), ctypes.sizeof(tag)):
            raise ctypes.WinError(ctypes.get_last_error())
        if tag.attributes & (0x400 | 0x10 | 1):
            raise ValueError("config must be a writable regular non-reparse file")
        # Read owner/group/DACL from the opened file, not another path lookup.
        security = config_security(handle)
        descriptor = msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY)
        handle = None  # descriptor now owns the handle
        with os.fdopen(descriptor, "rb") as source:
            opened = os.fstat(source.fileno())
            # CPython 3.12 lstat keeps creation time in st_ctime, whereas
            # fstat exposes change time. Compare birthtime across those APIs;
            # retain change-time checks between handle snapshots below/at commit.
            if (config_identity(opened, creation_time=True)
                    != config_identity(before, creation_time=True)):
                raise ValueError("config changed while reading; stop concurrent edits and retry")
            data = source.read()
            if config_identity(os.fstat(source.fileno())) != config_identity(opened):
                raise ValueError("config changed while reading; stop concurrent edits and retry")
        return opened, data, security
    finally:
        if handle is not None:
            KERNEL.CloseHandle(handle)


def config_staging(path, security):
    import msvcrt

    creation_security = None
    if security is not None:
        creation_security = ctypes.create_string_buffer(security.raw, len(security.raw))
        # Staging must not acquire extra inherited permissions from the parent.
        if not SECURITY.SetSecurityDescriptorControl(creation_security, 0x1000, 0x1000):
            raise ctypes.WinError(ctypes.get_last_error())
    attributes = SecurityAttributes(ctypes.sizeof(SecurityAttributes),
                                    ctypes.addressof(creation_security) if creation_security is not None else None,
                                    False)
    # Apply the original security descriptor at CREATE_NEW, before any data is
    # written. chmod on Windows cannot restrict inherited DACLs. No sharing;
    # GENERIC_WRITE|READ_CONTROL allows security readback on the held handle.
    handle = KERNEL.CreateFileW(str(path), 0x40020000, 0, ctypes.byref(attributes), 1, 0x80, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        if security is not None:
            control, revision = wintypes.WORD(), wintypes.DWORD()
            if not SECURITY.GetSecurityDescriptorControl(security, ctypes.byref(control), ctypes.byref(revision)):
                raise ctypes.WinError(ctypes.get_last_error())
            setter_security = ctypes.create_string_buffer(security.raw, len(security.raw))
            # Request the original auto-inherited state on a separate setter copy;
            # the request alone is no guarantee, so retain exact readback below.
            if control.value & 0x400:
                if not SECURITY.SetSecurityDescriptorControl(setter_security, 0x100, 0x100):
                    raise ctypes.WinError(ctypes.get_last_error())
            protection = 0x80000000 if control.value & 0x1000 else 0x20000000
            # Creation is not an exact descriptor copy. Restore the original
            # inheritance state on the still-empty, exclusively held file. Use
            # the low-level setter deliberately: SetSecurityInfo recomputes
            # inheritance, potentially adding parent ACEs/changing control bits.
            if not SECURITY.SetFileSecurityW(str(path), 4 | protection, setter_security):
                raise ctypes.WinError(ctypes.get_last_error())
            if config_security(handle).raw != security.raw:
                raise ValueError("staging owner/group/DACL or inheritance differs from config")
        return msvcrt.open_osfhandle(handle, os.O_WRONLY | os.O_BINARY)
    except BaseException:
        KERNEL.CloseHandle(handle)
        path.unlink(missing_ok=True)
        raise


def append_config(path, original, data, security, result):
    """Append through one exclusive existing-file handle; never set its ACL.

    This is not atomic against termination/power loss. Handled IO failures remove
    only our suffix when readback proves that truncation preserves original bytes.
    """
    import msvcrt

    if not isinstance(result, bytes) or len(result) <= len(data) or not result.startswith(data):
        raise ValueError("Windows in-place config update must only append bytes")
    # GENERIC_READ|GENERIC_WRITE, no sharing, OPEN_EXISTING, OPEN_REPARSE_POINT.
    # Refuse existing readers/writers/deleters and hold exclusion until verified.
    handle = KERNEL.CreateFileW(str(path), 0xc0000000, 0, None, 3, 0x00200000, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        descriptor = msvcrt.open_osfhandle(handle, os.O_RDWR | os.O_BINARY)
    except BaseException:
        KERNEL.CloseHandle(handle)
        raise
    with os.fdopen(descriptor, "r+b", buffering=0) as output:
        # Only size/edit timestamps may change as a result of our own append.
        identity = config_identity(original, creation_time=True)

        def verify(expected, *, unchanged=False):
            tag = AttributeTag()
            if not KERNEL.GetFileInformationByHandleEx(handle, 9, ctypes.byref(tag), ctypes.sizeof(tag)):
                raise ctypes.WinError(ctypes.get_last_error())
            opened = os.fstat(output.fileno())
            current = config_identity(opened, creation_time=True)
            if (tag.attributes & (0x400 | 0x10 | 1)
                    or current[:4] + current[-1:] != identity[:4] + identity[-1:]
                    or (unchanged and config_identity(opened) != config_identity(original))):
                raise ValueError("config identity/metadata changed on append handle")
            output.seek(0)
            contents = output.read()
            if (contents != expected or opened.st_size != len(expected)
                    or config_security(handle).raw != security.raw
                    or config_identity(os.fstat(output.fileno())) != config_identity(opened)):
                raise ValueError("config bytes/owner/group/DACL or metadata changed on append handle")

        verify(data, unchanged=True)
        output.seek(len(data))
        try:
            suffix = result[len(data):]
            if output.write(suffix) != len(suffix):
                raise OSError("short config append write")
            os.fsync(output.fileno())
            verify(result)
        except (OSError, ValueError) as error:
            try:
                output.seek(0)
                current = output.read()
                if not current.startswith(data) or not result.startswith(current):
                    raise ValueError("cannot safely remove an unrecognized append")
                verify(current)
                output.truncate(len(data))
                os.fsync(output.fileno())
                verify(data)
            except (OSError, ValueError) as recovery_error:
                raise OSError(f"in-place append failed; rollback incomplete or unverified at {path}; "
                              f"original length {len(data)} bytes, appended bytes may remain. "
                              "Stop editors and inspect bytes and owner/group/DACL before retrying: "
                              f"{recovery_error}") from error
            raise OSError(f"in-place append failed; original bytes restored at {path}: {error}") from error
    return True


def update_config(path, proposed):
    """Append opt-in config, preserving existing DACLs without conversion.

    Directory pins prevent ancestor swaps; the final file check/replace is not a
    lock against hostile actors with delete access. Never run concurrent editors.
    """
    path = Path(path)
    if (len(path.anchor) != 3 or path.anchor[1:] != ":\\" or not path.name
            or any(part in (".", "..") or part.endswith((" ", ".")) or ":" in part
                   or any(ord(c) < 32 or c in '<>"|?*' for c in part)
                   or part.split(".")[0].rstrip(" ").upper() in
                   {"CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$",
                    *(f"COM{i}" for i in "123456789¹²³"), *(f"LPT{i}" for i in "123456789¹²³")}
                   for part in path.parts[1:])):
        raise ValueError("Windows config requires a local absolute drive path without device/alias components")
    flags = wintypes.DWORD()
    if not KERNEL.GetVolumeInformationW(path.anchor, None, 0, None, None, ctypes.byref(flags), None, 0):
        raise ctypes.WinError(ctypes.get_last_error())
    if not flags.value & 8:  # FILE_PERSISTENT_ACLS
        raise ValueError("Windows config requires a filesystem with persistent ACLs")
    with ExitStack() as stack:
        parent = Path(path.anchor)
        stack.enter_context(pinned_directory(parent))
        for part in path.parts[1:-1]:
            parent /= part
            try:
                parent.mkdir()
            except FileExistsError:
                pass
            stack.enter_context(pinned_directory(parent))
        original, data, security = config_snapshot(path)
        result = proposed(data)
        if result is None:
            return False
        if security is not None:
            control, revision = wintypes.WORD(), wintypes.DWORD()
            if not SECURITY.GetSecurityDescriptorControl(security, ctypes.byref(control), ctypes.byref(revision)):
                raise ctypes.WinError(ctypes.get_last_error())
            # ReplaceFileW can broaden legacy DACLs through parent inheritance,
            # even when staging is an exact copy. Keep the existing file instead.
            if not control.value & 0x400:
                return append_config(path, original, data, security, result)
        staging = parent / (".crossroads-keybind-" + secrets.token_hex(12))
        recovery = None
        descriptor = config_staging(staging, security)
        try:
            with os.fdopen(descriptor, "wb") as output:
                output.write(result)
                output.flush()
                os.fsync(output.fileno())
            current, current_data, current_security = config_snapshot(path)
            if (config_identity(current) != config_identity(original) or current_data != data
                    or (current_security.raw if current_security is not None else None)
                    != (security.raw if security is not None else None)):
                raise ValueError("config changed before write; stop concurrent edits and retry")
            if original is None:
                os.rename(staging, path)  # Windows rename refuses a concurrent creation.
            else:
                # ReplaceFileW merges DACLs/streams; flags=0 never ignores ACL
                # merge failures. A private backup directory prevents backup-name
                # collisions, and retains original bytes on partial Win32 failures.
                candidate = parent / (".crossroads-keybind-recovery-" + secrets.token_hex(12))
                candidate.mkdir()
                recovery = candidate
                backup = recovery / "original.toml"
                if not KERNEL.ReplaceFileW(str(path), str(staging), str(backup), 0, None, None):
                    error = ctypes.get_last_error()
                    if backup.exists() or error in (1176, 1177):
                        retained = staging
                        staging, recovery = None, None
                        raise OSError(error, f"native replacement failed; recovery files retained: {retained}, {backup}")
                    raise ctypes.WinError(error)
                try:
                    _, _, final_security = config_snapshot(path)
                    if final_security is None or final_security.raw != security.raw:
                        raise ValueError("replacement descriptor differs from config")
                except (OSError, ValueError) as error:
                    retained = staging
                    staging, recovery = None, None
                    raise OSError("replacement security verification failed; recovery files retained: "
                                  f"{path}, {retained}, {backup}") from error
                backup.unlink()
            return True
        finally:
            if staging is not None:
                staging.unlink(missing_ok=True)
            if recovery is not None:
                recovery.rmdir()
