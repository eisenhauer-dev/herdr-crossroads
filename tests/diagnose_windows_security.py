"""Observational Win32 ACL probe. Only synthetic files in private temp directories.

Run natively via build-windows.py's 60-second supervisor, not as an acceptance
test. Raw descriptor/SDDL equality are diagnostics, not semantic-equivalence proofs.
"""
from contextlib import contextmanager, ExitStack
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import platform
import re
import runpy
import sys
import tempfile
import traceback


ROOT = Path(__file__).absolute().parents[1]
CASES = ("legacy-default-unprotected", "auto-inherited", "protected")
VARIANTS = ("dacl-only", "existing-protection-flags", "auto-inherit-request")


def emit(**record):
    print(json.dumps(record, sort_keys=True), flush=True)


def checkpoint(operation, **record):
    emit(stage="checkpoint", operation=operation, **record)


def failure(error):
    # No exception messages, filenames, locals or source lines: OS errors can
    # contain personal temp paths. Keep only code locations and numeric errors.
    errors = []
    while error is not None and len(errors) < 4:
        frames = []
        for frame, line in traceback.walk_tb(error.__traceback__):
            filename = os.path.abspath(frame.f_code.co_filename)
            root = os.path.normcase(str(ROOT)) + os.sep
            path = os.path.relpath(filename, str(ROOT)) if os.path.normcase(filename).startswith(root) else ""
            project = path.replace("\\", "/").startswith(("tests/", "scripts/"))
            frames.append({"file": path.replace("\\", "/") if project else "<external>",
                           "line": line, "function": frame.f_code.co_name if project else "<external>"})
        errors.append({"type": type(error).__name__, "winerror": getattr(error, "winerror", None),
                       "errno": getattr(error, "errno", None), "frames": frames[-16:]})
        error = error.__cause__ or (None if error.__suppress_context__ else error.__context__)
    return {"errors": errors}


def fixture_dacls(parent_sddl):
    """Construct the actual fixture strings; this does not validate Win32 acceptance."""
    owner = re.search(r"O:(.*?)(?=G:|D:|S:|$)", parent_sddl)
    if not owner or not owner[1]:
        raise ValueError("fixture parent owner missing from SDDL")
    grants = f"(A;;FA;;;{owner[1]})(A;;FA;;;SY)(A;;FA;;;BA)"
    return "D:P" + grants, "D:P" + grants.replace("(A;;", "(A;OICI;") + "(A;OICI;FR;;;WD)"


def observe(api, report=emit):
    """Keep API outcomes observational; setup/readback/cleanup must really work."""
    failures = 0
    for case in CASES:
        for variant in VARIANTS:
            labels = {"case": case, "variant": variant}
            report(**labels, stage="begin")
            try:
                with api.fixture(case) as fixture:
                    original = api.snapshot(fixture, "original")
                    report(**labels, stage="original", **original)
                    report(**labels, stage="controlled-parent", **api.snapshot(fixture, "parent"))
                    api.create_stage(fixture)
                    report(**labels, stage="staging-before-setter",
                           **api.snapshot(fixture, "staging", original))
                    report(**labels, stage="setter", **api.set_stage(fixture, variant))
                    report(**labels, stage="staging-after-setter",
                           **api.snapshot(fixture, "staging", original))
                    # Never write even synthetic stage bytes before both captures.
                    api.write_stage(fixture)
                    report(**labels, stage="replace-file-flags-0", **api.replace(fixture))
                    report(**labels, stage="target-after-replace",
                           **api.snapshot(fixture, "original", original))
            except (OSError, ValueError) as error:
                failures += 1
                report(**labels, stage="infrastructure-error", **failure(error))
            else:
                report(**labels, stage="cleanup-complete")
    report(stage="complete", infrastructure_failures=failures,
           acceptance="not evaluated")
    return int(bool(failures))


class Native:
    def __init__(self):
        checkpoint("load-native-bindings")
        self.files = runpy.run_path(str(ROOT / "scripts/windows-files.py"))
        self.kernel, self.security = self.files["KERNEL"], self.files["SECURITY"]
        self.security.ConvertStringSecurityDescriptorToSecurityDescriptorW.argtypes = [
            wintypes.LPCWSTR, wintypes.DWORD, ctypes.POINTER(ctypes.c_void_p),
            ctypes.POINTER(wintypes.DWORD)]
        self.security.ConvertStringSecurityDescriptorToSecurityDescriptorW.restype = wintypes.BOOL
        self.security.ConvertSecurityDescriptorToStringSecurityDescriptorW.argtypes = [
            ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
            ctypes.POINTER(ctypes.c_void_p), ctypes.POINTER(wintypes.DWORD)]
        self.security.ConvertSecurityDescriptorToStringSecurityDescriptorW.restype = wintypes.BOOL
        self.security.GetSecurityDescriptorLength.argtypes = [ctypes.c_void_p]
        self.security.GetSecurityDescriptorLength.restype = wintypes.DWORD
        self.security.GetSecurityDescriptorDacl.argtypes = [ctypes.c_void_p,
            ctypes.POINTER(wintypes.BOOL), ctypes.POINTER(ctypes.c_void_p),
            ctypes.POINTER(wintypes.BOOL)]
        self.security.GetSecurityDescriptorDacl.restype = wintypes.BOOL
        self.security.SetNamedSecurityInfoW.argtypes = [wintypes.LPWSTR, ctypes.c_int,
            wintypes.DWORD, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
        self.security.SetNamedSecurityInfoW.restype = wintypes.DWORD
        self.kernel.LocalFree.argtypes = [ctypes.c_void_p]
        self.kernel.LocalFree.restype = ctypes.c_void_p
        self.kernel.WriteFile.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD,
                                         ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
        self.kernel.WriteFile.restype = wintypes.BOOL
        self.kernel.FlushFileBuffers.argtypes = [wintypes.HANDLE]
        self.kernel.FlushFileBuffers.restype = wintypes.BOOL

    def checked(self, result):
        if not result:
            raise ctypes.WinError(ctypes.get_last_error())
        return result

    def free(self, pointer):
        checkpoint("LocalFree")
        if self.kernel.LocalFree(pointer):
            raise ctypes.WinError(ctypes.get_last_error())

    def descriptor(self, sddl):
        checkpoint("ConvertStringSecurityDescriptorToSecurityDescriptorW", fixture_sddl=sddl)
        pointer = ctypes.c_void_p()
        self.checked(self.security.ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl, 1, ctypes.byref(pointer), None))
        try:
            checkpoint("GetSecurityDescriptorLength")
            length = self.security.GetSecurityDescriptorLength(pointer)
            if not length:
                raise ValueError("empty converted descriptor")
            return ctypes.create_string_buffer(ctypes.string_at(pointer, length), length)
        finally:
            self.free(pointer)

    def describe(self, descriptor):
        checkpoint("GetSecurityDescriptorControl")
        control, revision = wintypes.WORD(), wintypes.DWORD()
        self.checked(self.security.GetSecurityDescriptorControl(
            descriptor, ctypes.byref(control), ctypes.byref(revision)))
        pointer = ctypes.c_void_p()
        checkpoint("ConvertSecurityDescriptorToStringSecurityDescriptorW")
        self.checked(self.security.ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor, 1, 7, ctypes.byref(pointer), None))
        try:
            sddl = ctypes.wstring_at(pointer)
        finally:
            self.free(pointer)
        return {"sddl_owner_group_dacl_aces_in_order": sddl,
                "control": hex(control.value), "dacl_protected_0x1000": bool(control.value & 0x1000),
                "dacl_auto_inherited_0x0400": bool(control.value & 0x400),
                "descriptor_revision": revision.value,
                "raw_length": len(descriptor.raw), "raw_hex": descriptor.raw.hex()}

    def open(self, path, *, create=False, descriptor=None, directory=False):
        checkpoint("CreateFileW", create=create, directory=directory)
        attributes = self.files["SecurityAttributes"](
            ctypes.sizeof(self.files["SecurityAttributes"]),
            ctypes.addressof(descriptor) if descriptor is not None else None, False)
        handle = self.kernel.CreateFileW(str(path), 0x40020000 if create else 0x20000,
                                        0 if create else 7,
                                        ctypes.byref(attributes) if create else None,
                                        1 if create else 3, 0x02000000 if directory else 0x80, None)
        if handle == ctypes.c_void_p(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        return handle

    def close(self, handle):
        checkpoint("CloseHandle")
        self.checked(self.kernel.CloseHandle(handle))

    def read(self, path, *, directory=False):
        handle = self.open(path, directory=directory)
        try:
            checkpoint("GetKernelObjectSecurity", directory=directory)
            return self.files["config_security"](handle)
        finally:
            self.close(handle)

    @contextmanager
    def pin(self, path):
        # Match production's no-DELETE-share pins, but check close failures too.
        checkpoint("pin-ancestor-CreateFileW")
        handle = self.kernel.CreateFileW(str(path), 0x81, 3, None, 3, 0x02200000, None)
        if handle == ctypes.c_void_p(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            tag = self.files["AttributeTag"]()
            checkpoint("pin-ancestor-GetFileInformationByHandleEx")
            self.checked(self.kernel.GetFileInformationByHandleEx(handle, 9, ctypes.byref(tag), ctypes.sizeof(tag)))
            if not tag.attributes & 0x10 or tag.attributes & 0x400:
                raise ValueError("fixture ancestor must be a non-reparse directory")
            yield
        finally:
            self.close(handle)

    @contextmanager
    def fixture(self, case):
        checkpoint("fixture-create-temporary-directory", case=case)
        with tempfile.TemporaryDirectory(prefix="crossroads-acl-diagnostic-") as temporary:
            parent = Path(temporary)
            flags = wintypes.DWORD()
            checkpoint("fixture-GetVolumeInformationW")
            self.checked(self.kernel.GetVolumeInformationW(
                parent.anchor, None, 0, None, None, ctypes.byref(flags), None, 0))
            if not flags.value & 8:
                raise ValueError("diagnostic requires persistent filesystem ACLs")
            with ExitStack() as pins:
                checkpoint("fixture-pin-ancestors")
                ancestor = Path(parent.anchor)
                pins.enter_context(self.pin(ancestor))
                for part in parent.parts[1:]:
                    ancestor /= part
                    pins.enter_context(self.pin(ancestor))
                checkpoint("fixture-read-initial-parent-security")
                parent_state = self.describe(self.read(parent, directory=True))
                checkpoint("fixture-construct-dacls", **parent_state)
                private_dacl, inheritable_dacl = fixture_dacls(parent_state["sddl_owner_group_dacl_aces_in_order"])
                descriptor = self.descriptor(private_dacl)
                checkpoint("fixture-SetFileSecurityW-private-parent", security_information="0x80000004")
                self.checked(self.security.SetFileSecurityW(
                    str(parent), 0x80000004, descriptor))
                target = parent / "original.fixture"
                if case == "legacy-default-unprotected":
                    checkpoint("fixture-create-legacy-original")
                    self.close(self.open(target, create=True))
                descriptor = self.descriptor(inheritable_dacl)
                checkpoint("fixture-SetFileSecurityW-inheritable-parent", security_information="0x80000004")
                self.checked(self.security.SetFileSecurityW(
                    str(parent), 0x80000004, descriptor))
                checkpoint("fixture-read-controlled-parent-security")
                parent_state = self.describe(self.read(parent, directory=True))
                checkpoint("fixture-build-parent-ace-marker", **parent_state)
                marker = self.describe(self.descriptor("D:(A;OICI;FR;;;WD)"))[
                    "sddl_owner_group_dacl_aces_in_order"].split("D:", 1)[1]
                if marker not in parent_state["sddl_owner_group_dacl_aces_in_order"]:
                    raise ValueError("controlled inheritable parent ACE not obtained: " + json.dumps(parent_state))
                if case != "legacy-default-unprotected":
                    checkpoint("fixture-create-original")
                    descriptor = self.descriptor(private_dacl)
                    self.close(self.open(target, create=True, descriptor=descriptor))
                    if case == "auto-inherited":
                        self.inherit_fixture(target, descriptor)
                checkpoint("fixture-write-synthetic-original")
                target.write_bytes(b"synthetic original\r\n")
                checkpoint("fixture-read-original-security")
                original = self.read(target)
                state = self.describe(original)
                checkpoint("fixture-check-control-prerequisites", **state)
                if (state["dacl_protected_0x1000"] != (case == "protected")
                        or (case == "auto-inherited" and not state["dacl_auto_inherited_0x0400"])
                        or (case == "legacy-default-unprotected" and state["dacl_auto_inherited_0x0400"])):
                    raise ValueError("fixture control prerequisites not obtained: " + json.dumps(state))
                fixture = {"original": target, "parent": parent,
                           "staging": parent / "staging.fixture", "security": original, "handle": None}
                try:
                    yield fixture
                finally:
                    checkpoint("fixture-close-staging-handle")
                    if fixture["handle"] is not None:
                        self.close(fixture["handle"])
                        fixture["handle"] = None
                checkpoint("fixture-close-ancestor-pins")
            # TemporaryDirectory cleanup occurs only after handles and pins close.
            checkpoint("fixture-cleanup-temporary-directory")

    def inherit_fixture(self, target, descriptor):
        # Do not assume CreateFile/SetFileSecurity established the current
        # inheritance model. Explicitly unprotect this target through the ACL API.
        present, defaulted, dacl = wintypes.BOOL(), wintypes.BOOL(), ctypes.c_void_p()
        checkpoint("fixture-GetSecurityDescriptorDacl")
        self.checked(self.security.GetSecurityDescriptorDacl(
            descriptor, ctypes.byref(present), ctypes.byref(dacl), ctypes.byref(defaulted)))
        if not present.value or not dacl.value:
            raise ValueError("fixture requires a present non-null DACL")
        checkpoint("fixture-SetNamedSecurityInfoW-auto-inheritance", security_information="0x20000004")
        error = self.security.SetNamedSecurityInfoW(
            str(target), 1, 0x20000004, None, None, dacl, None)
        if error:
            raise ctypes.WinError(error)

    def snapshot(self, fixture, name, original=None):
        descriptor = (self.files["config_security"](fixture["handle"])
                      if name == "staging" else self.read(fixture[name], directory=name == "parent"))
        result = self.describe(descriptor)
        if original is not None:
            result.update(raw_equal_to_original=result["raw_hex"] == original["raw_hex"],
                          sddl_equal_to_original=result["sddl_owner_group_dacl_aces_in_order"]
                          == original["sddl_owner_group_dacl_aces_in_order"])
        return result

    def create_stage(self, fixture):
        original = fixture["security"]
        creation = ctypes.create_string_buffer(original.raw, len(original.raw))
        self.checked(self.security.SetSecurityDescriptorControl(creation, 0x1000, 0x1000))
        fixture["handle"] = self.open(fixture["staging"], create=True, descriptor=creation)

    def set_stage(self, fixture, variant):
        flags = 4
        descriptor = fixture["security"]
        details = {}
        if variant == "auto-inherit-request":
            descriptor = ctypes.create_string_buffer(descriptor.raw, len(descriptor.raw))
            if self.describe(fixture["security"])["dacl_auto_inherited_0x0400"]:
                checkpoint("setter-SetSecurityDescriptorControl-auto-inherit-request")
                self.checked(self.security.SetSecurityDescriptorControl(descriptor, 0x100, 0x100))
            details["setter_descriptor"] = self.describe(descriptor)
        if variant in ("existing-protection-flags", "auto-inherit-request"):
            flags |= 0x80000000 if self.describe(fixture["security"])["dacl_protected_0x1000"] else 0x20000000
        result = self.security.SetFileSecurityW(str(fixture["staging"]), flags, descriptor)
        error = 0 if result else ctypes.get_last_error()
        return {"api": "SetFileSecurityW", "security_information": hex(flags),
                "success": bool(result), "winerror": error, **details}

    def write_stage(self, fixture):
        data = b"synthetic replacement\r\n"
        written = wintypes.DWORD()
        self.checked(self.kernel.WriteFile(fixture["handle"], data, len(data), ctypes.byref(written), None))
        if written.value != len(data):
            raise ValueError("short synthetic fixture write")
        self.checked(self.kernel.FlushFileBuffers(fixture["handle"]))
        self.close(fixture["handle"])
        fixture["handle"] = None

    def replace(self, fixture):
        result = self.kernel.ReplaceFileW(str(fixture["original"]), str(fixture["staging"]),
                                          str(fixture["parent"] / "backup.fixture"), 0, None, None)
        error = 0 if result else ctypes.get_last_error()
        return {"api": "ReplaceFileW", "flags": 0, "success": bool(result), "winerror": error}


def main():
    if os.name != "nt":
        emit(stage="skip", reason="native Win32 APIs unavailable; no ACL semantics tested")
        return 0
    emit(stage="native-start", python=platform.python_version(), windows=platform.win32_ver(),
         scope="temporary synthetic fixtures only; observational, not acceptance")
    try:
        return observe(Native())
    except (OSError, ValueError) as error:
        emit(stage="infrastructure-error", **failure(error))
        return 1


if __name__ == "__main__":
    sys.exit(main())
