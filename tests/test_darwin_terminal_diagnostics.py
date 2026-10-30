"""Diagnostic contracts; Linux execution is not native Darwin evidence."""
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import pty
import subprocess
import termios
import unittest
from unittest.mock import patch

import pty_keymaps

ROOT = Path(__file__).resolve().parents[1]


class DarwinTerminalDiagnosticsTests(unittest.TestCase):
    def module(self):
        path = ROOT / "tests/diagnose_darwin_terminal.py"
        self.assertTrue(path.exists(), "bounded native terminal diagnostic is required")
        spec = importlib.util.spec_from_file_location("diagnostic", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_no_reader_transition_uses_saved_stty_state_and_closes_descriptors(self):
        module = self.module()
        master, slave = pty.openpty()
        calls = []
        results = []
        snapshots = []
        original = subprocess.run
        get_attributes = termios.tcgetattr
        def run(argv, **kwargs):
            calls.append((argv, kwargs))
            result = original(argv, **kwargs)
            results.append(result)
            return result
        def attributes(fd):
            value = get_attributes(fd)
            snapshots.append(value)
            return value
        output = io.StringIO()
        with patch.object(module.pty, "openpty", return_value=(master, slave)), \
             patch.object(module.subprocess, "run", side_effect=run), \
             patch.object(module.termios, "tcgetattr", side_effect=attributes), \
             contextlib.redirect_stdout(output):
            module.transition("/bin/stty")
        self.assertEqual(len(calls), 3, "save, raw and restore commands must each run once")
        self.assertEqual(calls[0][0], ["/bin/stty", "-g"])
        self.assertEqual(calls[2][0], ["/bin/stty", results[0].stdout.strip()])
        self.assertEqual(calls[1][0][1:], ["raw", "-echo", "min", "0", "time", "1"])
        self.assertEqual(len(snapshots), 3)
        self.assertTrue(snapshots[0][3] & termios.ICANON)
        self.assertFalse(snapshots[1][3] & (termios.ICANON | termios.ECHO))
        self.assertTrue(snapshots[2][3] & termios.ICANON)
        self.assertTrue(all(c[1]["stdin"] == slave and c[1]["timeout"] == 2 and c[1]["check"] for c in calls))
        for label in ("before_raw", "raw", "immediately_restored", "no_reader", "differences"):
            self.assertIn(label, output.getvalue())
        for fd in (master, slave):
            with self.assertRaises(OSError):
                os.fstat(fd)

    def test_command_failure_is_not_reported_as_an_observation(self):
        module = self.module()
        for error, effects in ((subprocess.CalledProcessError(2, ["stty"], stderr="failed"), []),
                               (subprocess.TimeoutExpired(["stty"], 2), [subprocess.CompletedProcess([], 0, "saved"),
                                                                        subprocess.CompletedProcess([], 0, "")])):
            master, slave = pty.openpty()
            output = io.StringIO()
            with patch.object(module.pty, "openpty", return_value=(master, slave)), \
                 patch.object(module.subprocess, "run", side_effect=effects + [error]), \
                 contextlib.redirect_stdout(output), self.assertRaises(type(error)):
                module.transition("stty")
            self.assertNotIn("immediately_restored", output.getvalue())
            for fd in (master, slave):
                with self.assertRaises(OSError):
                    os.fstat(fd)

    def test_darwin_metadata_records_selected_and_system_stty_without_environment(self):
        module = self.module()
        output = io.StringIO()
        with patch.object(module.sys, "platform", "darwin"), \
             patch.object(module.shutil, "which", return_value="/bin/stty"), \
             patch.object(module, "command", return_value="native metadata") as command, \
             patch.object(module, "transition") as transition, \
             contextlib.redirect_stdout(output):
            module.main()
        transition.assert_called_once_with("/bin/stty")
        self.assertIn(["/usr/sbin/sysctl", "-n", "kern.osrelease", "kern.osversion", "kern.version"],
                      [c.args[0] for c in command.call_args_list])
        self.assertIn(["/usr/bin/sw_vers"], [c.args[0] for c in command.call_args_list])
        self.assertIn(["/usr/bin/what", "/bin/stty"], [c.args[0] for c in command.call_args_list])
        for field in ("python", "uname", "selected_stty", "system_stty", "sha256", "realpath", "source_provenance"):
            self.assertIn(field, output.getvalue())
        self.assertNotIn("HERDR_", output.getvalue())

    def test_required_metadata_error_propagates_but_optional_file_error_is_labelled(self):
        module = self.module()
        error = subprocess.CalledProcessError(1, ["tool"], stderr="metadata failed")
        with patch.object(module.sys, "platform", "darwin"), \
             patch.object(module, "command", side_effect=error), \
             contextlib.redirect_stdout(io.StringIO()), self.assertRaises(subprocess.CalledProcessError):
            module.main()
        output = io.StringIO()
        with patch.object(module, "command", side_effect=error), contextlib.redirect_stdout(output):
            module.identity("system_stty", "/bin/stty")
        self.assertIn("system_stty_file_unavailable", output.getvalue())
        self.assertIn("exit status 1", output.getvalue())

    def test_pending_difference_is_reported_without_reading_writing_or_flushing_input(self):
        module = self.module()
        saved = [1, 2, 3, 1483, 9600, 9600, [b"\x03"]]
        restored = saved.copy()
        restored[3] |= 0x20000000
        output = io.StringIO()
        with patch.object(module, "command", side_effect=["saved", "", ""]) as commands, \
             patch.object(module.termios, "tcgetattr", side_effect=[saved, [0] * 7, restored]) as attributes, \
             patch.object(module.os, "read") as read, patch.object(module.os, "write") as write, \
             patch.object(module.termios, "tcflush") as flush, contextlib.redirect_stdout(output):
            module.transition("stty")
        self.assertEqual(commands.call_count, 3)
        self.assertEqual(attributes.call_count, 3)
        for action in (read, write, flush):
            action.assert_not_called()
        self.assertIn("differences={'lflag': (1483, 536872395)}", output.getvalue())

    def test_non_darwin_skips_only_diagnostic(self):
        module = self.module()
        output = io.StringIO()
        with patch.object(module.sys, "platform", "linux"), \
             patch.object(module, "transition") as transition, contextlib.redirect_stdout(output):
            module.main()
        transition.assert_not_called()
        self.assertIn("SKIP", output.getvalue())

    def test_missing_cli_failure_logs_observation_stages_without_weakening_equality(self):
        saved = [1, 2, 3, 1483, 9600, 9600, [b"\x03"]]
        actual = saved.copy()
        actual[3] |= 0x20000000
        raw = b"\x1b[?1000h\x1b[?1006h\x1b[?1000l\x1b[?1006l\x1b[?25hError: missing\r\n"
        master, slave = pty.openpty()
        output = io.StringIO()
        with patch.object(pty_keymaps.pty, "openpty", return_value=(master, slave)), \
             patch.object(pty_keymaps.sys, "platform", "darwin"), \
             patch.object(pty_keymaps.termios, "tcgetattr", side_effect=[saved, actual, actual, actual, actual]), \
             patch.object(pty_keymaps.subprocess, "Popen") as child, \
             patch.object(pty_keymaps, "drain", return_value=raw), \
             contextlib.redirect_stderr(output), self.assertRaises(AssertionError):
            child.return_value.wait.return_value = 1
            pty_keymaps.startup_failure_case("binary", ROOT)
        for stage in ("before_app", "after_wait", "after_drain", "raw_state=unobserved", repr(actual)):
            self.assertIn(stage, output.getvalue())
        for fd in (master, slave):
            with self.assertRaises(OSError):
                os.fstat(fd)


if __name__ == "__main__":
    unittest.main()
