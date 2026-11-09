"""Native Windows host-config safety, confined to temporary directories."""
import ctypes
import os
from pathlib import Path
import runpy
import stat
import struct
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]


class WindowsSnapshotMetadataTests(unittest.TestCase):
    """Model CPython Windows metadata without requiring Win32 on the test host."""

    def setUp(self):
        with mock.patch.object(ctypes, "WinDLL", create=True):
            self.helper = runpy.run_path(str(ROOT / "scripts/windows-files.py"))
        self.before = self.info(st_ctime_ns=100)
        self.opened = self.info(st_ctime_ns=200)
        self.path = mock.Mock()
        self.path.lstat.return_value = self.before
        self.source = mock.MagicMock()
        self.source.__enter__.return_value = self.source
        self.source.read.return_value = b"# original\r\n"
        self.helper["KERNEL"].CreateFileW.return_value = 123

        def control(descriptor, bits, revision):
            bits._obj.value = 0x8404
            revision._obj.value = 1
            return 1
        self.helper["SECURITY"].GetSecurityDescriptorControl.side_effect = control

        def security(handle, flags, buffer, length, needed):
            needed._obj.value = 4
            return 1
        self.helper["SECURITY"].GetKernelObjectSecurity.side_effect = security

    def info(self, **changes):
        return SimpleNamespace(**{
            **dict(st_dev=7, st_ino=11, st_mode=stat.S_IFREG | 0o666,
                   st_nlink=1, st_size=12, st_mtime_ns=150, st_ctime_ns=200,
                   st_birthtime_ns=100, st_atime_ns=175, st_file_attributes=0x20),
            **changes,
        })

    def snapshot(self, opened=None, after=None):
        opened = self.opened if opened is None else opened
        after = opened if after is None else after
        with mock.patch.dict(sys.modules, msvcrt=mock.Mock()), \
                mock.patch.object(os, "O_BINARY", 0, create=True), \
                mock.patch.object(os, "fdopen", return_value=self.source), \
                mock.patch.object(os, "fstat", side_effect=[opened, after]):
            return self.helper["config_snapshot"](self.path)

    def test_creation_time_from_path_matches_handle_despite_distinct_change_time(self):
        opened, data, security = self.snapshot(after=self.info(st_atime_ns=300))
        self.assertIs(opened, self.opened)
        self.assertEqual(data, b"# original\r\n")
        self.assertEqual(security.raw, bytes(4))
        self.helper["KERNEL"].CreateFileW.assert_called_once_with(
            str(self.path), 0x80000000, 1, None, 3, 0x00200000, None)

    def test_legacy_and_future_consistent_timestamp_semantics_are_accepted(self):
        del self.before.st_birthtime_ns
        legacy = self.info(st_ctime_ns=100)
        del legacy.st_birthtime_ns
        self.assertEqual(self.snapshot(legacy)[1], b"# original\r\n")
        self.path.lstat.return_value = self.info()
        self.assertEqual(self.snapshot()[1], b"# original\r\n")

    def test_swapped_or_changed_file_before_open_is_refused(self):
        for field in ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size",
                      "st_mtime_ns", "st_birthtime_ns"):
            with self.subTest(field=field):
                changed = self.info(**{field: getattr(self.opened, field) + 1})
                with self.assertRaisesRegex(ValueError, "config changed while reading"):
                    self.snapshot(changed)
        self.source.read.assert_not_called()

    def test_change_time_and_other_edit_metadata_during_read_are_still_checked(self):
        for field in ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size",
                      "st_mtime_ns", "st_ctime_ns"):
            with self.subTest(field=field):
                changed = self.info(**{field: getattr(self.opened, field) + 1})
                with self.assertRaisesRegex(ValueError, "config changed while reading"):
                    self.snapshot(after=changed)
                self.assertNotEqual(self.helper["config_identity"](self.opened),
                                    self.helper["config_identity"](changed))

    def test_unsafe_path_metadata_is_refused_before_open(self):
        for changes in ({"st_nlink": 2}, {"st_file_attributes": 1},
                        {"st_file_attributes": 0x400}, {"st_mode": stat.S_IFDIR | 0o777}):
            with self.subTest(changes=changes):
                self.path.lstat.return_value = self.info(**changes)
                with self.assertRaisesRegex(ValueError, "single-link regular non-reparse"):
                    self.snapshot()
        self.helper["KERNEL"].CreateFileW.assert_not_called()

    def test_change_time_bytes_and_acl_changes_before_commit_preserve_target(self):
        security = ctypes.create_string_buffer(b"acl")
        original = (self.opened, b"# original\r\n", security)

        def volume(anchor, name, size, serial, maximum, flags, filesystem, length):
            flags._obj.value = 8  # FILE_PERSISTENT_ACLS
            return 1
        self.helper["KERNEL"].GetVolumeInformationW.side_effect = volume
        with tempfile.TemporaryDirectory(
                dir=None if os.name == "nt" else "/tmp/opencode") as directory:
            parent = Path(directory)
            target = parent / "config.toml"
            target.write_bytes(original[1])
            selected = SimpleNamespace(anchor="C:\\", name=target.name,
                                       parts=("C:\\", target.name))
            for current in ((self.info(st_ctime_ns=201), original[1], security),
                            (self.opened, b"# modified\r\n", security),
                            (self.opened, original[1], ctypes.create_string_buffer(b"new"))):
                with self.subTest(current=current), mock.patch.dict(
                        self.helper["update_config"].__globals__,
                        Path=mock.Mock(side_effect=[selected, parent]),
                        pinned_directory=mock.MagicMock(),
                        config_snapshot=mock.Mock(side_effect=[original, current]),
                        config_staging=lambda path, acl: os.open(
                            path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)):
                    with self.assertRaisesRegex(ValueError, "config changed before write"):
                        self.helper["update_config"](selected, lambda data: data + b"# appended\n")
                self.assertEqual(target.read_bytes(), original[1])
                self.assertEqual(list(parent.glob(".crossroads-keybind-*")), [])
        self.helper["KERNEL"].ReplaceFileW.assert_not_called()


class WindowsCommitSecurityTests(unittest.TestCase):
    """Exercise replacement and recovery with real isolated files, mocked Win32."""

    def setUp(self):
        with mock.patch.object(ctypes, "WinDLL", create=True):
            self.helper = runpy.run_path(str(ROOT / "scripts/windows-files.py"))
        self.temporary = tempfile.TemporaryDirectory(
            dir=None if os.name == "nt" else "/tmp/opencode")
        self.addCleanup(self.temporary.cleanup)
        self.parent = Path(self.temporary.name)
        self.target = self.parent / "config.toml"
        self.data = b"# original\r\n"
        self.target.write_bytes(self.data)
        self.security = ctypes.create_string_buffer(b"exact owner/group/DACL")
        self.original = (self.target.stat(), self.data, self.security)
        self.selected = mock.MagicMock(anchor="C:\\", name=self.target.name,
                                       parts=("C:\\", self.target.name))
        self.selected.name = self.target.name
        self.selected.__str__.return_value = str(self.target)
        self.selected.__fspath__.return_value = str(self.target)
        self.control = 0x8404
        self.final_reads = 0

        def control(descriptor, bits, revision):
            bits._obj.value = self.control
            revision._obj.value = 1
            return 1

        def volume(anchor, name, size, serial, maximum, flags, filesystem, length):
            flags._obj.value = 8
            return 1

        def replace(target, staged, backup, flags, exclude, reserved):
            self.assertEqual(flags, 0)
            os.rename(self.target, backup)
            os.rename(staged, self.target)
            return 1

        self.helper["SECURITY"].GetSecurityDescriptorControl.side_effect = control
        self.helper["KERNEL"].GetVolumeInformationW.side_effect = volume
        self.helper["KERNEL"].ReplaceFileW.side_effect = replace
        self.stage = mock.Mock(side_effect=lambda path, acl: os.open(
            path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600))
        self.transform = mock.Mock(return_value=self.data + b"# appended\n")

    def update(self, final=None):
        def snapshot(path):
            if self.helper["KERNEL"].ReplaceFileW.called:
                self.final_reads += 1
                backups = list(self.parent.rglob("original.toml"))
                self.assertEqual(len(backups), 1, "verify before deleting the backup")
                self.assertEqual(backups[0].read_bytes(), self.data)
                if isinstance(final, Exception):
                    raise final
                return self.target.stat(), self.target.read_bytes(), final or self.security
            return self.original

        with mock.patch.dict(self.helper["update_config"].__globals__,
                             Path=mock.Mock(side_effect=[self.selected, self.parent]),
                             pinned_directory=mock.MagicMock(),
                             config_snapshot=mock.Mock(side_effect=snapshot),
                             config_staging=self.stage):
            return self.helper["update_config"](self.selected, self.transform)

    def test_legacy_unprotected_and_protected_descriptors_refused_before_staging(self):
        for control in (0x8004, 0x9004):
            with self.subTest(control=hex(control)):
                self.control = control
                with self.assertRaisesRegex(ValueError, "auto-inherited DACL"):
                    self.update()
                self.assertEqual(self.target.read_bytes(), self.data)
                self.assertEqual(self.target.stat(), self.original[0])
                self.assertEqual(self.security.raw, self.original[2].raw)
                self.assertEqual(list(self.parent.glob(".crossroads-keybind-*")), [])
                self.stage.assert_not_called()
                self.helper["KERNEL"].ReplaceFileW.assert_not_called()

    def test_legacy_noop_does_not_require_conversion_or_staging(self):
        self.control = 0x8004
        self.transform.return_value = None
        self.assertFalse(self.update())
        self.stage.assert_not_called()
        self.helper["SECURITY"].GetSecurityDescriptorControl.assert_not_called()
        self.assertEqual(self.target.stat(), self.original[0])

    def test_final_descriptor_mismatch_retains_original_without_retry_or_restore(self):
        with self.assertRaisesRegex(OSError, "verification failed; recovery files retained") as error:
            self.update(ctypes.create_string_buffer(b"broader DACL"))
        self.assert_recovery(error)

    def test_final_descriptor_read_failure_retains_original_without_false_success(self):
        with self.assertRaisesRegex(OSError, "verification failed; recovery files retained") as error:
            self.update(OSError("read failure"))
        self.assert_recovery(error)

    def assert_recovery(self, error):
        backups = list(self.parent.rglob("original.toml"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_bytes(), self.data)
        self.assertEqual(self.target.read_bytes(), self.transform.return_value)
        self.assertIn(str(backups[0]), str(error.exception))
        self.assertIn(str(self.target), str(error.exception))
        self.helper["KERNEL"].ReplaceFileW.assert_called_once()

    def test_exact_final_descriptor_is_verified_before_backup_deletion(self):
        self.assertTrue(self.update())
        self.assertEqual(self.final_reads, 1)
        self.assertEqual(self.target.read_bytes(), self.transform.return_value)
        self.assertEqual(list(self.parent.glob(".crossroads-keybind-*")), [])
        self.helper["KERNEL"].ReplaceFileW.assert_called_once()

    def test_protected_auto_inherited_descriptor_keeps_exact_success_contract(self):
        self.control = 0x9404
        self.assertTrue(self.update())
        self.assertEqual(self.final_reads, 1)
        self.assertEqual(list(self.parent.glob(".crossroads-keybind-*")), [])

    def test_control_read_failure_refuses_before_staging(self):
        self.helper["SECURITY"].GetSecurityDescriptorControl.return_value = 0
        self.helper["SECURITY"].GetSecurityDescriptorControl.side_effect = None
        with mock.patch.object(ctypes, "WinError", side_effect=OSError("control failure"), create=True), \
                mock.patch.object(ctypes, "get_last_error", return_value=5, create=True):
            with self.assertRaisesRegex(OSError, "control failure"):
                self.update()
        self.stage.assert_not_called()
        self.helper["KERNEL"].ReplaceFileW.assert_not_called()
        self.assertEqual(self.target.stat(), self.original[0])

    def test_missing_config_creation_still_inherits_parent_security(self):
        self.target.unlink()
        self.original = (None, b"", None)
        self.assertTrue(self.update())
        self.helper["SECURITY"].GetSecurityDescriptorControl.assert_not_called()
        self.helper["KERNEL"].ReplaceFileW.assert_not_called()
        self.assertEqual(self.target.read_bytes(), self.transform.return_value)
        self.assertEqual(list(self.parent.glob(".crossroads-keybind-*")), [])

    def test_partial_native_failure_keeps_original_and_stage(self):
        def partial(target, staged, backup, flags, exclude, reserved):
            self.assertEqual(flags, 0)
            os.rename(self.target, backup)
            return 0
        self.helper["KERNEL"].ReplaceFileW.side_effect = partial
        with mock.patch.object(ctypes, "get_last_error", return_value=1177, create=True):
            with self.assertRaisesRegex(OSError, "native replacement failed; recovery files retained"):
                self.update()
        backups = list(self.parent.rglob("original.toml"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_bytes(), self.data)
        staged = [path for path in self.parent.glob(".crossroads-keybind-*") if path.is_file()]
        self.assertEqual(len(staged), 1)
        self.assertEqual(staged[0].read_bytes(), self.transform.return_value)
        self.assertFalse(self.target.exists())
        self.assertEqual(self.final_reads, 0)


class WindowsStagingSecurityTests(unittest.TestCase):
    """Model creation-time inheritance separately from exact descriptor restoration."""

    def setUp(self):
        with mock.patch.object(ctypes, "WinDLL", create=True):
            self.helper = runpy.run_path(str(ROOT / "scripts/windows-files.py"))
        self.api = self.helper["SECURITY"]
        self.kernel = self.helper["KERNEL"]
        self.path = mock.Mock()
        self.crt = mock.Mock()
        self.crt.open_osfhandle.return_value = 456
        self.created = None
        self.restored = None

        def control(descriptor, interest, value):
            old = struct.unpack_from("<H", descriptor.raw, 2)[0]
            ctypes.memmove(ctypes.addressof(descriptor) + 2,
                           struct.pack("<H", (old & ~interest) | value), 2)
            return 1

        def create(path, access, share, attributes, disposition, flags, template):
            self.assertEqual((share, disposition), (0, 1))
            self.assertTrue(access & 0x20000, "staging descriptor must be readable for verification")
            pointer = attributes._obj.descriptor
            if pointer:
                # CreateFile processes a supplied descriptor through inheritance;
                # protection prevents parent grants, but is not an exact SD copy.
                self.created = bytearray(ctypes.string_at(pointer, self.length))
                bits = struct.unpack_from("<H", self.created, 2)[0]
                self.assertTrue(bits & 0x1000, "creation must exclude extra parent grants")
                struct.pack_into("<H", self.created, 2, bits | 0x400)
            return 123

        def restore(path, flags, descriptor):
            self.restored = (path, flags, descriptor.raw)
            self.created = bytearray(descriptor.raw)
            # Model the native probe's missing-request AUTO_INHERITED bit loss;
            # AUTO_INHERIT_REQ is consumed, not persisted on the file.
            bits = struct.unpack_from("<H", self.created, 2)[0]
            restored = bits & ~0x100
            if not bits & 0x100:
                restored &= ~0x400
            struct.pack_into("<H", self.created, 2, restored)
            return 1

        def get_control(descriptor, control, revision):
            control._obj.value = struct.unpack_from("<H", descriptor.raw, 2)[0]
            revision._obj.value = 1
            return 1

        def read(handle, flags, buffer, length, needed):
            self.assertEqual((handle, flags), (123, 7))
            needed._obj.value = len(self.created)
            if buffer is not None:
                ctypes.memmove(buffer, bytes(self.created), len(self.created))
            return 1

        self.api.SetSecurityDescriptorControl.side_effect = control
        self.api.SetFileSecurityW.side_effect = restore
        self.api.GetSecurityDescriptorControl.side_effect = get_control
        self.api.GetKernelObjectSecurity.side_effect = read
        self.kernel.CreateFileW.side_effect = create

    def descriptor(self, control, inherited=False):
        owner = struct.pack("<BB6sI", 1, 1, bytes([0, 0, 0, 0, 0, 5]), 18)
        group = struct.pack("<BB6sII", 1, 2, bytes([0, 0, 0, 0, 0, 5]), 32, 544)
        ace = struct.pack("<BBHI", 0, 0x10 if inherited else 0, 8 + len(owner), 0x1f01ff) + owner
        acl = struct.pack("<BBHHH", 2, 0, 8 + len(ace), 1, 0) + ace
        raw = struct.pack("<BBHIIII", 1, 0, control, 20, 20 + len(owner), 0,
                          20 + len(owner) + len(group)) + owner + group + acl
        self.length = len(raw)
        return ctypes.create_string_buffer(raw, len(raw))

    def stage(self, security):
        with mock.patch.dict(sys.modules, msvcrt=self.crt), \
                mock.patch.object(os, "O_BINARY", 0, create=True):
            return self.helper["config_staging"](self.path, security)

    def test_exact_owner_group_dacl_and_inheritance_restored_before_descriptor_handoff(self):
        for control, inherited in ((0x8004, False), (0x8404, True),
                                   (0x9004, False), (0x9404, False)):
            with self.subTest(control=hex(control)):
                security = self.descriptor(control, inherited)
                original = security.raw

                def handoff(handle, flags):
                    self.assertEqual(bytes(self.created), original,
                                     "staging must retain exact owner/group/DACL and inheritance")
                    return 456

                self.crt.open_osfhandle.side_effect = handoff
                self.assertEqual(self.stage(security), 456)
                self.assertEqual(security.raw, original, "do not mutate the target snapshot")
                flags = 0x80000004 if control & 0x1000 else 0x20000004
                setter = bytearray(original)
                if control & 0x400:
                    struct.pack_into("<H", setter, 2, control | 0x100)
                self.assertEqual(self.restored, (str(self.path), flags, bytes(setter)))
                setter_input = self.api.SetFileSecurityW.call_args.args[2]
                self.assertNotEqual(ctypes.addressof(setter_input), ctypes.addressof(security))
                controls = self.api.SetSecurityDescriptorControl.call_args_list
                self.assertEqual([(call.args[1], call.args[2]) for call in controls],
                                 [(0x1000, 0x1000)] + ([(0x100, 0x100)] if control & 0x400 else []))
                self.assertNotEqual(ctypes.addressof(controls[0].args[0]),
                                    ctypes.addressof(setter_input))
                self.api.SetSecurityDescriptorControl.reset_mock()

    def test_auto_inherit_request_failure_preserves_snapshot_and_cleans_empty_stage(self):
        security = self.descriptor(0x8404, inherited=True)
        original = security.raw
        control = self.api.SetSecurityDescriptorControl.side_effect

        def fail_request(descriptor, interest, value):
            return 0 if interest == 0x100 else control(descriptor, interest, value)

        self.api.SetSecurityDescriptorControl.side_effect = fail_request
        with mock.patch.object(ctypes, "WinError", side_effect=OSError("request failure"), create=True), \
                mock.patch.object(ctypes, "get_last_error", return_value=5, create=True):
            with self.assertRaisesRegex(OSError, "request failure"):
                self.stage(security)
        self.assertEqual(security.raw, original)
        self.api.SetFileSecurityW.assert_not_called()
        self.crt.open_osfhandle.assert_not_called()
        self.kernel.CloseHandle.assert_called_once_with(123)
        self.path.unlink.assert_called_once_with(missing_ok=True)

    def test_restore_failure_or_inexact_readback_closes_and_removes_empty_staging(self):
        security = self.descriptor(0x8004)
        restore = self.api.SetFileSecurityW.side_effect
        for failure in ("restore", "readback", "mismatch", "handoff"):
            with self.subTest(failure=failure):
                self.kernel.CloseHandle.reset_mock()
                self.path.unlink.reset_mock()
                self.crt.open_osfhandle.reset_mock()
                with mock.patch.object(ctypes, "WinError", side_effect=OSError("native ACL failure"),
                                       create=True), \
                        mock.patch.object(ctypes, "get_last_error", return_value=5, create=True):
                    if failure == "restore":
                        self.api.SetFileSecurityW.side_effect = lambda *args: 0
                    else:
                        self.api.SetFileSecurityW.side_effect = restore
                    if failure == "readback":
                        self.api.GetKernelObjectSecurity.return_value = 0
                        read = self.api.GetKernelObjectSecurity.side_effect
                        self.api.GetKernelObjectSecurity.side_effect = None
                    if failure == "mismatch":
                        self.api.SetFileSecurityW.side_effect = lambda *args: 1
                    self.crt.open_osfhandle.side_effect = OSError("handoff failure")
                    with self.assertRaises((OSError, ValueError)):
                        self.stage(security)
                    if failure == "readback":
                        self.api.GetKernelObjectSecurity.side_effect = read
                if failure != "handoff":
                    self.crt.open_osfhandle.assert_not_called()
                self.kernel.CloseHandle.assert_called_once_with(123)
                self.path.unlink.assert_called_once_with(missing_ok=True)

    def test_new_config_uses_parent_security_without_restore(self):
        self.assertEqual(self.stage(None), 456)
        self.api.SetSecurityDescriptorControl.assert_not_called()
        self.api.SetFileSecurityW.assert_not_called()
        self.api.GetKernelObjectSecurity.assert_not_called()


@unittest.skipUnless(os.name == "nt", "requires Windows file sharing, reparse points and ACLs")
class WindowsKeybindTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="crossroads-keybind-native-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.path = self.root / "配置 with spaces" / "config.toml"
        self.path.parent.mkdir()
        self.path.write_bytes(b"# original\r\n")
        self.helper = runpy.run_path(str(ROOT / "scripts/windows-files.py"))
        from windows_security_fixture import auto_inherit
        auto_inherit(self.path)

    def update(self, transform=lambda data: data + b"# appended\r\n"):
        self.helper["update_config"](self.path, transform)

    def test_legacy_fixture_refused_without_bytes_security_identity_or_staging_changes(self):
        probe = runpy.run_path(str(ROOT / "tests/diagnose_windows_security.py"))
        with mock.patch.dict(probe["Native"].__init__.__globals__, checkpoint=lambda *a, **k: None):
            api = probe["Native"]()
            with api.fixture("legacy-default-unprotected") as fixture:
                path = fixture["original"]
                before, data, security = self.helper["config_snapshot"](path)
                self.assertFalse(struct.unpack_from("<H", security.raw, 2)[0] & 0x400)
                with mock.patch.object(self.helper["KERNEL"], "ReplaceFileW") as replace, \
                        mock.patch.dict(self.helper["update_config"].__globals__,
                                        config_staging=mock.Mock()) as patched:
                    with self.assertRaisesRegex(ValueError, "auto-inherited DACL"):
                        self.helper["update_config"](path, lambda data: data + b"# appended\n")
                    replace.assert_not_called()
                    patched["config_staging"].assert_not_called()
                after, current, current_security = self.helper["config_snapshot"](path)
                self.assertEqual(self.helper["config_identity"](after),
                                 self.helper["config_identity"](before))
                self.assertEqual(current, data)
                self.assertEqual(current_security.raw, security.raw)
                self.assertEqual(list(path.parent.glob(".crossroads-keybind-*")), [])

    def test_pinned_parent_cannot_be_renamed_while_staging(self):
        def transform(data):
            with self.assertRaises(OSError):
                self.path.parent.rename(self.root / "moved")
            return data + b"# appended\n"
        self.update(transform)
        self.assertEqual(self.path.read_bytes(), b"# original\r\n# appended\n")

    def test_junction_ancestor_and_directory_target_are_refused(self):
        junction = self.root / "junction"
        subprocess.run(["cmd.exe", "/d", "/c", "mklink", "/J", str(junction), str(self.path.parent)],
                       check=True, capture_output=True)
        self.addCleanup(junction.rmdir)
        for path in (junction / self.path.name, self.path.parent):
            with self.subTest(path=path), self.assertRaises((ValueError, OSError)):
                self.helper["update_config"](path, lambda data: data + b"# bad\n")
        self.assertEqual(self.path.read_bytes(), b"# original\r\n")

    def test_hardlinks_readonly_and_existing_writer_are_refused(self):
        link = self.root / "hardlink.toml"
        os.link(self.path, link)
        with self.assertRaisesRegex(ValueError, "single-link"):
            self.update()
        link.unlink()
        self.path.chmod(0o444)
        try:
            with self.assertRaises((ValueError, OSError)):
                self.update()
        finally:
            self.path.chmod(0o666)
        with self.path.open("ab"):
            with self.assertRaises(OSError):
                self.update()
        self.assertEqual(self.path.read_bytes(), b"# original\r\n")

    def test_reparse_target_is_refused_even_without_symlink_privilege(self):
        # A directory junction is a reparse target and needs no Developer Mode.
        target = self.root / "target"
        target.mkdir()
        self.path.unlink()
        subprocess.run(["cmd.exe", "/d", "/c", "mklink", "/J", str(self.path), str(target)],
                       check=True, capture_output=True)
        self.addCleanup(self.path.rmdir)
        with self.assertRaises((ValueError, OSError)):
            self.update()
        self.assertEqual(list(target.iterdir()), [])

    def test_network_device_and_alias_paths_fail_before_access(self):
        for path in (r"\\server\share\config.toml", r"\\?\C:\config.toml",
                     str(self.root / "config.toml:stream"), str(self.root / "config.toml."),
                     str(self.root / "NUL")):
            with self.subTest(path=path), self.assertRaises(ValueError):
                self.helper["update_config"](Path(path), lambda data: b"# bad\n")

    def test_explicit_protected_dacl_survives_replacement(self):
        def security_descriptor():
            from ctypes import wintypes
            api = ctypes.WinDLL("advapi32", use_last_error=True)
            api.GetFileSecurityW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, ctypes.c_void_p,
                                            wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
            api.GetFileSecurityW.restype = wintypes.BOOL
            needed = wintypes.DWORD()
            api.GetFileSecurityW(str(self.path), 7, None, 0, ctypes.byref(needed))
            buffer = ctypes.create_string_buffer(needed.value)
            self.assertTrue(api.GetFileSecurityW(str(self.path), 7, buffer, needed, ctypes.byref(needed)))
            return buffer.raw

        real_fsync = os.fsync

        def check_staging(descriptor):
            staged = next(self.path.parent.glob(".crossroads-keybind-*"))
            original_path = self.path
            try:
                self.path = staged
                self.assertEqual(security_descriptor(), before,
                                 "staging must not expose config under a broader DACL")
            finally:
                self.path = original_path
            real_fsync(descriptor)

        default = security_descriptor()
        before = default
        with mock.patch.object(os, "fsync", side_effect=check_staging):
            self.update()
        self.assertEqual(security_descriptor(), default, "default owner/group/DACL must survive")

        # Add an inheritable SYSTEM grant in the owned fixture, then explicitly
        # enable inheritance to cover an additional auto-inherited DACL.
        subprocess.run(["icacls", str(self.path.parent), "/grant", "*S-1-5-18:(OI)(CI)(F)"],
                       check=True, capture_output=True)
        subprocess.run(["icacls", str(self.path), "/inheritance:e"], check=True, capture_output=True)
        before = security_descriptor()
        self.assertTrue(struct.unpack_from("<H", before, 2)[0] & 0x400)
        with mock.patch.object(os, "fsync", side_effect=check_staging):
            self.update()
        self.assertEqual(security_descriptor(), before, "inherited owner/group/DACL must survive")

        subprocess.run(["icacls", str(self.path), "/inheritance:d"], check=True, capture_output=True)
        before = security_descriptor()
        self.assertEqual(struct.unpack_from("<H", before, 2)[0] & 0x1400, 0x1400,
                         "protected success fixture must retain modern inheritance")
        with mock.patch.object(os, "fsync", side_effect=check_staging):
            self.update()
        self.assertEqual(security_descriptor(), before)

    def test_repeated_auto_inherited_updates_preserve_exact_staging_and_target_security(self):
        import msvcrt

        subprocess.run(["icacls", str(self.path.parent), "/grant", "*S-1-5-18:(OI)(CI)(F)"],
                       check=True, capture_output=True)
        subprocess.run(["icacls", str(self.path), "/inheritance:e"], check=True, capture_output=True)
        _, original, security = self.helper["config_snapshot"](self.path)
        self.assertTrue(struct.unpack_from("<H", security.raw, 2)[0] & 0x400)
        real_fsync = os.fsync

        def check_staging(descriptor):
            staged_security = self.helper["config_security"](msvcrt.get_osfhandle(descriptor))
            self.assertEqual(staged_security.raw, security.raw,
                             "exclusively held staging must preserve the original descriptor")
            real_fsync(descriptor)

        for iteration in range(3):
            with self.subTest(iteration=iteration), \
                    mock.patch.object(os, "fsync", side_effect=check_staging):
                self.update()
                _, data, current = self.helper["config_snapshot"](self.path)
                self.assertEqual(data, original + b"# appended\r\n" * (iteration + 1))
                self.assertEqual(current.raw, security.raw)
                self.assertEqual(list(self.path.parent.glob(".crossroads-keybind-*")), [])

    def test_partial_native_failure_retains_recoverable_original(self):
        def partial_failure(target, staged, backup, flags, exclude, reserved):
            self.assertEqual(flags, 0, "ACL/merge errors must not be suppressed")
            os.rename(target, backup)
            return 0
        with mock.patch.object(self.helper["KERNEL"], "ReplaceFileW", side_effect=partial_failure), \
                mock.patch("ctypes.get_last_error", return_value=1177):
            with self.assertRaisesRegex(OSError, "recovery files retained"):
                self.update()
        backups = list(self.root.rglob("original.toml"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_bytes(), b"# original\r\n")
        staged = [p for p in self.path.parent.glob(".crossroads-keybind-*") if p.is_file()]
        self.assertEqual(len(staged), 1)
        self.assertEqual(staged[0].read_bytes(), b"# original\r\n# appended\r\n")

    def test_replacement_failure_does_not_fall_back_to_truncation(self):
        with mock.patch.object(self.helper["KERNEL"], "ReplaceFileW", return_value=0), \
                mock.patch("ctypes.get_last_error", return_value=5):
            with self.assertRaises(OSError):
                self.update()
        self.assertEqual(self.path.read_bytes(), b"# original\r\n")
        self.assertEqual(list(self.root.rglob(".crossroads-keybind-*")), [])


if __name__ == "__main__":
    unittest.main()
