"""Portable diagnostic orchestration contracts; not models of Win32 ACL semantics."""
from contextlib import contextmanager, nullcontext, redirect_stdout
import ctypes
import io
import json
import os
from pathlib import Path
import runpy
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).absolute().parents[1]
PROBE = runpy.run_path(str(ROOT / "tests/diagnose_windows_security.py"))


class Harness:
    def __init__(self, fail=None):
        self.fail = fail
        self.events = []

    @contextmanager
    def fixture(self, case):
        self.events.append("setup:" + case)
        if self.fail == "setup":
            raise OSError("fixture setup failed")
        try:
            yield case
        finally:
            self.events.append("cleanup")
            if self.fail == "cleanup":
                raise OSError("fixture cleanup failed")

    def snapshot(self, fixture, name, original=None):
        self.events.append("snapshot:" + name)
        if self.fail == "readback" and name == "staging":
            raise OSError("capture failed")
        return {"fixture_record": name, "comparison_requested": original is not None}

    def create_stage(self, fixture):
        self.events.append("create")

    def set_stage(self, fixture, variant):
        self.events.append("setter:" + variant)
        return {"success": False, "winerror": 5}

    def write_stage(self, fixture):
        self.events.append("write")

    def replace(self, fixture):
        self.events.append("replace")
        return {"success": False, "winerror": 32}


class DiagnosticTests(unittest.TestCase):
    def test_exactly_three_variants_keep_all_existing_fixture_cases(self):
        self.assertEqual(PROBE["CASES"],
                         ("legacy-default-unprotected", "auto-inherited", "protected"))
        self.assertEqual(PROBE["VARIANTS"],
                         ("dacl-only", "existing-protection-flags", "auto-inherit-request"))

    def test_setter_request_uses_separate_copy_and_only_requests_existing_auto_inheritance(self):
        for control in (0x8004, 0x8404, 0x9004, 0x9404):
            for variant in ("dacl-only", "existing-protection-flags", "auto-inherit-request"):
                with self.subTest(control=hex(control), variant=variant):
                    native = PROBE["Native"].__new__(PROBE["Native"])
                    native.security = unittest.mock.Mock()
                    original = struct.pack("<BBH", 1, 0, control) + b"unchanged owner/group/DACL"
                    descriptor = ctypes.create_string_buffer(original, len(original))
                    events = []

                    def describe(buffer):
                        return {"control": hex(struct.unpack_from("<H", buffer.raw, 2)[0]),
                                "dacl_protected_0x1000": bool(control & 0x1000),
                                "dacl_auto_inherited_0x0400": bool(control & 0x400),
                                "raw_hex": buffer.raw.hex()}

                    def request(buffer, interest, value):
                        events.append("request")
                        self.assertIsNot(buffer, descriptor)
                        self.assertEqual(buffer.raw, original)
                        self.assertEqual((interest, value), (0x100, 0x100))
                        ctypes.memmove(ctypes.addressof(buffer) + 2,
                                       struct.pack("<H", control | 0x100), 2)
                        return 1

                    def setter(path, flags, buffer):
                        events.append("setter")
                        self.assertEqual(path, "staging.fixture")
                        protection = 0x80000000 if control & 0x1000 else 0x20000000
                        self.assertEqual(flags, 4 if variant == "dacl-only" else 4 | protection)
                        expected = control | 0x100 if variant == "auto-inherit-request" and control & 0x400 else control
                        self.assertEqual(buffer.raw, original[:2] + struct.pack("<H", expected) + original[4:])
                        if variant == "auto-inherit-request":
                            self.assertIsNot(buffer, descriptor)
                        return 1

                    native.describe = describe
                    native.security.SetSecurityDescriptorControl.side_effect = request
                    native.security.SetFileSecurityW.side_effect = setter
                    with patch.dict(native.checked.__func__.__globals__, checkpoint=lambda *a, **kw: None):
                        record = native.set_stage({"security": descriptor, "staging": Path("staging.fixture")}, variant)
                    self.assertEqual(descriptor.raw, original)
                    self.assertEqual(events, (["request"] if variant == "auto-inherit-request" and control & 0x400 else [])
                                     + ["setter"])
                    self.assertTrue(record["success"])
                    self.assertEqual(record["winerror"], 0)
                    if variant == "auto-inherit-request":
                        expected = control | 0x100 if control & 0x400 else control
                        self.assertEqual(record["setter_descriptor"]["control"], hex(expected))
                        self.assertEqual(record["setter_descriptor"]["raw_hex"],
                                         (original[:2] + struct.pack("<H", expected) + original[4:]).hex())

    def test_request_control_failure_is_infrastructure_error_before_file_setter(self):
        native = PROBE["Native"].__new__(PROBE["Native"])
        native.security = unittest.mock.Mock()
        descriptor = ctypes.create_string_buffer(b"original")
        native.describe = lambda buffer: {"dacl_protected_0x1000": False,
                                          "dacl_auto_inherited_0x0400": True}
        native.security.SetSecurityDescriptorControl.return_value = 0
        with patch.dict(native.checked.__func__.__globals__, checkpoint=lambda *a, **kw: None), \
                patch.object(ctypes, "WinError", side_effect=OSError("control failure"), create=True), \
                patch.object(ctypes, "get_last_error", return_value=5, create=True):
            with self.assertRaises(OSError):
                native.set_stage({"security": descriptor, "staging": Path("staging.fixture")},
                                 "auto-inherit-request")
        self.assertEqual(descriptor.raw, b"original\0")
        native.security.SetFileSecurityW.assert_not_called()

    def test_native_fixture_explicitly_converts_only_auto_inherited_target(self):
        for case, converted in (("legacy-default-unprotected", False), ("auto-inherited", True),
                                ("protected", False), ("auto-inherited", False)):
            with self.subTest(case=case, converted=converted), tempfile.TemporaryDirectory(
                    dir=None if os.name == "nt" else "/tmp/opencode") as directory:
                native = PROBE["Native"].__new__(PROBE["Native"])
                native.kernel = unittest.mock.Mock()
                native.security = unittest.mock.Mock()
                native.pin = lambda path: nullcontext()
                native.open = unittest.mock.Mock(return_value=123)
                native.close = unittest.mock.Mock()
                native.read = unittest.mock.Mock(return_value="read-descriptor")
                native.descriptor = unittest.mock.Mock(return_value="explicit-descriptor")
                state = {"sddl_owner_group_dacl_aces_in_order": "O:SYD:P(A;OICI;FR;;;WD)",
                         "dacl_protected_0x1000": case == "protected",
                         "dacl_auto_inherited_0x0400": False}

                def volume(anchor, name, size, serial, maximum, flags, filesystem, length):
                    flags._obj.value = 8
                    return 1

                def dacl(descriptor, present, pointer, defaulted):
                    self.assertEqual(descriptor, "explicit-descriptor")
                    present._obj.value = True
                    pointer._obj.value = 1234
                    return 1

                def convert(path, kind, flags, owner, group, acl, sacl):
                    self.assertEqual(path, str(Path(directory) / "original.fixture"))
                    self.assertFalse(Path(path).exists(), "conversion must precede synthetic writes")
                    native.close.assert_called_once_with(123)
                    self.assertEqual((kind, flags, owner, group, acl.value, sacl),
                                     (1, 0x20000004, None, None, 1234, None))
                    state["dacl_auto_inherited_0x0400"] = converted
                    return 0  # DWORD ERROR_SUCCESS, not BOOL success

                native.kernel.GetVolumeInformationW.side_effect = volume
                native.security.GetSecurityDescriptorDacl.side_effect = dacl
                native.security.SetNamedSecurityInfoW.side_effect = convert
                native.describe = lambda descriptor: dict(state)
                with patch.dict(native.checked.__func__.__globals__,
                                checkpoint=lambda *args, **kwargs: None), \
                        patch.object(PROBE["tempfile"], "TemporaryDirectory",
                                     return_value=nullcontext(directory)):
                    if case == "auto-inherited" and not converted:
                        with self.assertRaisesRegex(ValueError, "fixture control prerequisites"):
                            with native.fixture(case):
                                self.fail("missing auto-inherited control bit must fail setup")
                    else:
                        with native.fixture(case) as fixture:
                            self.assertEqual(fixture["security"], "read-descriptor")
                self.assertEqual(native.security.SetNamedSecurityInfoW.call_count,
                                 int(case == "auto-inherited"))
                if case != "legacy-default-unprotected":
                    native.open.assert_called_once_with(Path(directory) / "original.fixture",
                                                        create=True, descriptor="explicit-descriptor")

    def test_fixture_inheritance_rejects_missing_null_dacl_and_native_errors(self):
        native = PROBE["Native"].__new__(PROBE["Native"])
        native.security = unittest.mock.Mock()
        for failure in ("missing", "null", "get-error", "set-error"):
            with self.subTest(failure=failure):
                def dacl(descriptor, present, pointer, defaulted):
                    present._obj.value = failure != "missing"
                    pointer._obj.value = None if failure == "null" else 1234
                    return int(failure != "get-error")
                native.security.GetSecurityDescriptorDacl.side_effect = dacl
                native.security.SetNamedSecurityInfoW.reset_mock()
                native.security.SetNamedSecurityInfoW.return_value = 5
                with patch.dict(native.checked.__func__.__globals__,
                                checkpoint=lambda *args, **kwargs: None), \
                        patch.object(ctypes, "WinError", side_effect=OSError("native failure"),
                                     create=True) as win_error, \
                        patch.object(ctypes, "get_last_error", return_value=1338, create=True):
                    with self.assertRaises((OSError, ValueError)):
                        native.inherit_fixture(Path("original.fixture"), "descriptor")
                if failure == "set-error":
                    win_error.assert_called_once_with(5)
                else:
                    native.security.SetNamedSecurityInfoW.assert_not_called()

    def test_actual_fixture_dacl_construction_preserves_owner_and_ace_fields(self):
        # String construction only: no assertion that a Windows API accepts it.
        for owner in ("S-1-5-21-111-222-333-1001", "SY", "BA"):
            for suffix in ("", "G:SY", "G:BAD:PAI(A;OICI;FA;;;BA)", "D:(A;;FR;;;WD)S:"):
                with self.subTest(owner=owner, suffix=suffix):
                    private, inheritable = PROBE["fixture_dacls"]("O:" + owner + suffix)
                    self.assertEqual(private, f"D:P(A;;FA;;;{owner})(A;;FA;;;SY)(A;;FA;;;BA)")
                    self.assertEqual(inheritable, f"D:P(A;OICI;FA;;;{owner})(A;OICI;FA;;;SY)"
                                     "(A;OICI;FA;;;BA)(A;OICI;FR;;;WD)")
                    for dacl in (private, inheritable):
                        for ace in dacl[4:-1].split(")("):
                            self.assertEqual(len(ace.split(";")), 6)
        for missing in ("", "G:BAD:(A;;FA;;;SY)", "O:G:SYD:P"):
            with self.subTest(missing=missing), self.assertRaisesRegex(ValueError, "owner missing"):
                PROBE["fixture_dacls"](missing)

    def test_infrastructure_tracebacks_keep_code_locations_and_codes_not_private_data(self):
        try:
            try:
                exec(compile("raise OSError(1336, 'private-message', '/private-home/fixture')",
                             "/private-home/native-caller.py", "exec"))
            except OSError:
                raise ValueError("cleanup-message /private-home/fixture")
        except ValueError as error:
            record = PROBE["failure"](error)
        payload = json.dumps(record)
        for hidden in ("private-home", "private-message", "cleanup-message", "native-caller.py"):
            self.assertNotIn(hidden, payload)
        self.assertEqual([item["type"] for item in record["errors"]], ["ValueError", "OSError"])
        self.assertEqual(record["errors"][1]["errno"], 1336)
        self.assertEqual(record["errors"][1]["frames"][-1]["file"], "<external>")
        self.assertIn("tests/test_windows_security_diagnostic.py", payload)
        self.assertTrue(all(item["frames"] for item in record["errors"]))

    def test_api_failure_observations_do_not_stop_variants_or_claim_acceptance(self):
        api, records = Harness(), []
        self.assertEqual(PROBE["observe"](api, lambda **record: records.append(record)), 0)
        self.assertEqual(api.events, [event
            for case in PROBE["CASES"] for variant in PROBE["VARIANTS"]
            for event in ("setup:" + case, "snapshot:original", "snapshot:parent", "create",
                          "snapshot:staging", "setter:" + variant, "snapshot:staging", "write",
                          "replace", "snapshot:original", "cleanup")])
        for case in PROBE["CASES"]:
            for variant in PROBE["VARIANTS"]:
                stages = [record["stage"] for record in records
                          if record.get("case") == case and record.get("variant") == variant]
                self.assertEqual(stages, ["begin", "original", "controlled-parent",
                    "staging-before-setter", "setter", "staging-after-setter",
                    "replace-file-flags-0", "target-after-replace", "cleanup-complete"])
        self.assertEqual(records[-1], {"stage": "complete", "infrastructure_failures": 0,
                                       "acceptance": "not evaluated"})

    def test_setup_readback_and_cleanup_failures_are_real_and_other_probes_continue(self):
        count = len(PROBE["CASES"]) * len(PROBE["VARIANTS"])
        for failure in ("setup", "readback", "cleanup"):
            with self.subTest(failure=failure):
                api, records = Harness(failure), []
                self.assertEqual(PROBE["observe"](api, lambda **record: records.append(record)), 1)
                self.assertEqual(records[-1]["infrastructure_failures"], count)
                self.assertEqual(sum(r["stage"] == "begin" for r in records), count)
                self.assertEqual(sum(r["stage"] == "cleanup-complete" for r in records), 0)
                errors = [r for r in records if r["stage"] == "infrastructure-error"]
                self.assertEqual(len(errors), count)
                for record in errors:
                    self.assertEqual(record["errors"][0]["type"], "OSError")
                    self.assertTrue(record["errors"][0]["frames"])
                self.assertEqual(api.events.count("cleanup"), 0 if failure == "setup" else count)
                if failure == "readback":
                    self.assertNotIn("write", api.events)
                    self.assertNotIn("replace", api.events)

    def test_dispatch_skips_before_constructing_native_api_and_reports_native_setup_error(self):
        main = PROBE["main"]
        for name in ("posix", "nt"):
            output = io.StringIO()
            with patch.dict(main.__globals__, Native=unittest.mock.Mock(side_effect=OSError("API unavailable"))), \
                    patch.object(PROBE["os"], "name", name), redirect_stdout(output):
                self.assertEqual(main(), 0 if name == "posix" else 1)
                native = main.__globals__["Native"]
                self.assertEqual(native.call_count, 0 if name == "posix" else 1)
            records = [json.loads(line) for line in output.getvalue().splitlines()]
            self.assertEqual(records[-1]["stage"], "skip" if name == "posix" else "infrastructure-error")
            if name == "posix":
                self.assertIn("no ACL semantics tested", records[-1]["reason"])

    def test_records_are_json_and_flushed(self):
        with patch.dict(PROBE["emit"].__globals__, print=unittest.mock.Mock()) as namespace:
            PROBE["emit"](stage="example", raw_hex="0001")
            args, kwargs = namespace["print"].call_args
        self.assertEqual(json.loads(args[0]), {"stage": "example", "raw_hex": "0001"})
        self.assertEqual(kwargs, {"flush": True})

    def test_fixture_checkpoints_identify_operations_without_paths(self):
        records = []
        checkpoint = PROBE["checkpoint"]
        with patch.dict(checkpoint.__globals__, emit=lambda **record: records.append(record)):
            checkpoint("fixture-SetFileSecurityW-private-parent", security_information="0x80000004")
        self.assertEqual(records, [{"stage": "checkpoint", "operation": "fixture-SetFileSecurityW-private-parent",
                                    "security_information": "0x80000004"}])

    @unittest.skipIf(sys.platform == "win32", "portable subprocess skip contract only")
    def test_portable_entry_point_explicitly_skips_native_apis(self):
        result = subprocess.run([sys.executable, "-B", "tests/diagnose_windows_security.py"],
                                cwd=ROOT, capture_output=True, text=True, timeout=10, check=True)
        self.assertEqual(json.loads(result.stdout)["stage"], "skip")
        self.assertEqual(result.stderr, "")


if __name__ == "__main__":
    unittest.main()
