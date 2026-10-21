"""Windows-only installation: pin directories against rename; reject reparse points."""

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


class AttributeTag(ctypes.Structure):
    _fields_ = [("attributes", wintypes.DWORD), ("tag", wintypes.DWORD)]


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
