"""Explicit modern inheritance for synthetic Windows success fixtures only."""

import ctypes
from ctypes import wintypes
from pathlib import Path
import runpy


def auto_inherit(path):
    helper = runpy.run_path(str(Path(__file__).resolve().parents[1] / "scripts/windows-files.py"))
    _, _, descriptor = helper["config_snapshot"](path)
    api = helper["SECURITY"]
    api.GetSecurityDescriptorDacl.argtypes = [ctypes.c_void_p, ctypes.POINTER(wintypes.BOOL),
        ctypes.POINTER(ctypes.c_void_p), ctypes.POINTER(wintypes.BOOL)]
    api.GetSecurityDescriptorDacl.restype = wintypes.BOOL
    api.SetNamedSecurityInfoW.argtypes = [wintypes.LPWSTR, ctypes.c_int, wintypes.DWORD,
        ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
    api.SetNamedSecurityInfoW.restype = wintypes.DWORD
    present, defaulted, dacl = wintypes.BOOL(), wintypes.BOOL(), ctypes.c_void_p()
    if not api.GetSecurityDescriptorDacl(descriptor, ctypes.byref(present),
                                         ctypes.byref(dacl), ctypes.byref(defaulted)):
        raise ctypes.WinError(ctypes.get_last_error())
    if not present.value or not dacl.value:
        raise ValueError("fixture requires a present non-null DACL")
    error = api.SetNamedSecurityInfoW(str(path), 1, 0x20000004, None, None, dacl, None)
    if error:
        raise ctypes.WinError(error)
    _, _, descriptor = helper["config_snapshot"](path)
    control, revision = wintypes.WORD(), wintypes.DWORD()
    if not api.GetSecurityDescriptorControl(descriptor, ctypes.byref(control), ctypes.byref(revision)):
        raise ctypes.WinError(ctypes.get_last_error())
    if not control.value & 0x400:
        raise ValueError("fixture did not acquire an auto-inherited DACL")
