"""Closed-pipe EOF and real-PTY lifecycle regressions."""
import errno
import os
import pty
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import tty
import unittest
from pathlib import Path
from unittest.mock import patch

import pty_keymaps


class ReaderEofTests(unittest.TestCase):
    def check_reader(self, expression, payload=b"", expected=None, error=None):
        code = f"""
import os
import sys
import time
sys.dont_write_bytecode = True
sys.path.insert(0, {str(Path(__file__).resolve().parent)!r})
from pty_render import frame
from pty_keymaps import read_until, read_frame, drain, read_search_escape
reader, writer = os.pipe()
os.write(writer, {payload!r})
os.close(writer)
expected_error = {error!r}
try:
    try:
        result = {expression}
    except AssertionError as failure:
        assert expected_error is not None and expected_error in str(failure), str(failure)
    else:
        assert expected_error is None, 'expected EOF failure'
        assert result == {expected!r}, repr(result)
finally:
    os.close(reader)
"""
        try:
            result = subprocess.run([sys.executable, "-c", code], capture_output=True,
                                    text=True, timeout=5)
        except subprocess.TimeoutExpired:
            self.fail(f"reader did not terminate at EOF: {expression}")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_render_eof_before_marker(self):
        self.check_reader("frame(reader, b'end')", b"partial", error="EOF")

    def test_render_eof_after_marker(self):
        self.check_reader("frame(reader, b'end')", b"end tail", "end tail")

    def test_keymap_eof_before_marker(self):
        self.check_reader("read_until(reader, b'end')", b"partial", error="EOF")

    def test_keymap_marker_before_eof(self):
        self.check_reader("read_until(reader, b'end')", b"end", b"end")

    def test_keymap_frame_eof_after_marker(self):
        self.check_reader("read_frame(reader)", b"frame\x1b[?25h", "frame\x1b[?25h")

    def test_keymap_expected_frame_eof(self):
        self.check_reader("read_frame(reader, expected=lambda raw: False)", error="EOF")

    def test_escape_frame_eof(self):
        self.check_reader("read_search_escape(reader, time.monotonic(), 'codex', [])", b"\x1b[H", error="EOF")

    def test_drain_eof(self):
        self.check_reader("drain(reader)", b"tail\x1b[?25h", b"tail\x1b[?25h")

    def test_drain_empty_eof(self):
        self.check_reader("drain(reader)", expected=b"")


class EscapeObservationTests(unittest.TestCase):
    @staticmethod
    def frame(searching=True, query="codex", cursor=None, help_search=False):
        header = pty_keymaps.expected_search_row(80, query, "Help" if help_search else "Search" if searching else "Select")
        footer = "  Enter/Esc: return · Backspace: erase" if searching else "q: close keys" if help_search else "q: close"
        if cursor is None:
            cursor = b"\x1b[1;16H\x1b[?25h" if searching else b"\x1b[?25l"
        return (f"\x1b[H\x1b[J{header}\x1b[2;1H  > codex\x1b[15;1H{footer}".encode() + cursor)

    def observe(self, emissions, real_pty=False, help_search=False):
        if real_pty:
            reader, writer = pty.openpty()
            tty.setraw(writer)
            self.addCleanup(os.close, reader)
            self.addCleanup(os.close, writer)
        else:
            pair = socket.socketpair()
            for endpoint in pair:
                self.addCleanup(endpoint.close)
            reader, writer = (endpoint.fileno() for endpoint in pair)
        start = time.monotonic()
        stop = threading.Event()
        errors = []

        def emit():
            try:
                # The repaint represents output queued before Esc, delivered late.
                self.assertEqual(os.read(writer, 1), b"\x1b")
                for at, payload in emissions:
                    if stop.wait(max(0, start + at - time.monotonic())):
                        return
                    os.write(writer, payload)
            except BaseException as error:
                errors.append(error)

        worker = threading.Thread(target=emit)
        worker.start()
        try:
            os.write(reader, b"\x1b")
            lines, _ = pty_keymaps.screen(self.frame(help_search=help_search).decode(), 80, 15)
            return pty_keymaps.read_search_escape(reader, start, "codex", lines)
        finally:
            stop.set()
            worker.join(timeout=1)
            self.assertFalse(worker.is_alive())
            if errors:
                raise errors[0]

    def test_delayed_search_repaint_is_not_escape_resolution(self):
        for real_pty in (False, True):
            with self.subTest(real_pty=real_pty):
                raw = self.observe([(0.025, self.frame()), (0.105, self.frame(False))], real_pty)
                self.assertEqual(raw, self.frame(False).decode())

    def test_early_select_is_rejected_even_after_search_repaint(self):
        for emissions in (
            [(0.025, self.frame(False))],
            [(0.025, self.frame() + self.frame(False))],
            [(0.025, self.frame()), (0.05, self.frame(False))],
            [(0.08, self.frame(False))],
        ):
            reason = "Esc transition timing" if emissions[-1][0] == 0.08 else "Esc resolved before its deadline"
            with self.subTest(emissions=emissions), self.assertRaisesRegex(AssertionError, reason):
                self.observe(emissions)

    def test_fragmented_search_and_cursor_marker_cross_guard_boundary(self):
        search = self.frame()
        raw = self.observe([(0.025, search[:-1]), (0.08, search[-1:]), (0.105, self.frame(False))])
        self.assertEqual(raw, self.frame(False).decode())

    def test_early_select_body_with_late_cursor_marker_is_rejected(self):
        for help_search, repaint in ((False, b""), (False, self.frame()), (True, self.frame(help_search=True))):
            with self.subTest(help_search=help_search, search_repaint=bool(repaint)):
                navigation = self.frame(False, help_search=help_search)
                emissions = [(0.01, repaint), (0.025, navigation[:-1]), (0.105, navigation[-1:])]
                received = iter((at, bytes([byte])) for at, payload in emissions for byte in payload)
                now = 0.0

                def read(fd, size):
                    nonlocal now
                    now, byte = next(received)
                    return byte

                lines, _ = pty_keymaps.screen(self.frame(help_search=help_search).decode(), 80, 15)
                with patch.object(pty_keymaps.time, "monotonic", side_effect=lambda: now), \
                     patch.object(pty_keymaps.select, "select", return_value=([123], [], [])), \
                     patch.object(pty_keymaps.os, "read", side_effect=read), \
                     self.assertRaisesRegex(AssertionError, "Esc resolved before its deadline"):
                    pty_keymaps.read_search_escape(123, 0.0, "codex", lines)

    def test_fast_send_waits_for_matching_complete_selection_without_quiet_gap(self):
        reader, writer = socket.socketpair()
        self.addCleanup(reader.close)
        self.addCleanup(writer.close)
        popup = pty_keymaps.Popup.__new__(pty_keymaps.Popup)
        popup.master, popup.width, popup.height = reader.fileno(), 80, 15
        intermediate = self.frame(False).replace(b"  > codex", b"  > Claude")
        final = self.frame(False)
        writer.sendall(intermediate + final)
        expected, _ = pty_keymaps.screen(final.decode(), 80, 15)
        ready = pty_keymaps.select.select

        def select_without_gap(readers, writers, errors, timeout):
            self.assertNotEqual(timeout, 0.01, "fast timing includes an artificial output gap")
            return ready(readers, writers, errors, timeout)

        with patch.object(pty_keymaps.select, "select", side_effect=select_without_gap):
            actual = popup.send(b"\x1b\x1b[A", expected=lambda raw: pty_keymaps.screen(raw, 80, 15)[0] == expected)
        self.assertEqual(actual, expected)
        self.assertEqual(writer.recv(4), b"\x1b\x1b[A")

    def test_coalesced_search_and_select_are_checked_separately(self):
        raw = self.observe([(0.105, self.frame() + self.frame(False))])
        self.assertEqual(raw, self.frame(False).decode())

    def test_help_delayed_search_is_allowed_but_navigation_preserves_contents(self):
        search = self.frame(help_search=True)
        navigation = self.frame(False, help_search=True)
        raw = self.observe([(0.025, search[:-1]), (0.08, search[-1:]), (0.105, navigation)], help_search=True)
        self.assertEqual(raw, navigation.decode())
        for invalid in (navigation.replace(b"codex", b"codeq"), navigation.replace(b"  >", b"   "), navigation.replace(b"close keys", b"close pane")):
            with self.subTest(invalid=invalid), self.assertRaises(AssertionError):
                self.observe([(0.105, invalid)], help_search=True)

    def test_search_query_footer_and_cursor_must_remain_intact(self):
        for repaint in (
            self.frame(query="codeq"),
            self.frame().replace(b"Backspace: erase", b"q: close"),
            self.frame(cursor=b"\x1b[1;15H\x1b[?25h"),
            self.frame(cursor=b"\x1b[?25l"),
            self.frame().replace(b"  > codex", b"    codex"),
        ):
            with self.subTest(repaint=repaint), self.assertRaises(AssertionError):
                self.observe([(0.025, repaint), (0.105, self.frame(False))])

    def test_transition_requires_complete_valid_select_before_original_deadline(self):
        for emissions in (
            [],
            [(0.24, self.frame(False))],
            [(0.105, self.frame(False)[:-1])],
            [(0.105, self.frame(False, query="codeq"))],
            [(0.105, self.frame(False, cursor=b"\x1b[1;16H\x1b[?25h"))],
            [(0.105, self.frame(False).replace(b"  > codex", b"    codex"))],
        ):
            with self.subTest(emissions=emissions), self.assertRaises(AssertionError):
                self.observe(emissions)


class PopupLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-lifecycle-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.binary = self.directory / "child"
        self.fds = []
        openpty = pty.openpty

        def record_pty():
            pair = openpty()
            self.fds.extend(pair)
            return pair

        self.patcher = patch.object(pty_keymaps.pty, "openpty", side_effect=record_pty)
        self.patcher.start()
        self.addCleanup(self.patcher.stop)

    def child(self, kitty=False, status=0, restore=True, initial=True, linger=False, repaint=False):
        first_frame = ((b"\x1b[>1u" if kitty else b"") + b"frame\x1b[?25l") if initial else b"bad frame\x1b[?25l"
        tail = (b"\x1b[<u" if kitty else b"") + (b"\x1b[?25h" if restore else b"")
        self.binary.write_text(f"""#!{sys.executable}
import os
import tty
tty.setraw(0)
os.write(1, b'\\x1b[?u')
if {kitty!r}:
    assert os.read(0, 5) == b'\\x1b[?0u'
else:
    assert os.read(0, 1) == b'!'
os.write(1, {first_frame!r})
key = os.read(0, 1)
if {linger!r}:
    os.read(0, 1)
if {repaint!r}:
    assert key == b'\\x1b' and os.read(0, 1) == b'\\x1b'
    pending = b'x' * 262144
    while pending:
        pending = pending[os.write(1, pending):]
os.write(1, {tail!r})
raise SystemExit({status})
""")
        self.binary.chmod(0o700)

    def popup(self, **kwargs):
        probe = b"" if kwargs.get("kitty") else b"!"
        popup = pty_keymaps.Popup(str(self.binary), self.directory, probe_input=probe, **kwargs)
        self.addCleanup(popup.abort)
        return popup

    def assert_closed(self):
        for fd in self.fds:
            with self.assertRaises(OSError) as error:
                os.fstat(fd)
            self.assertEqual(error.exception.errno, errno.EBADF)

    def test_exit_tail_is_drained_with_parent_slave_open(self):
        for kitty, status in [(False, 0), (True, 7)]:
            with self.subTest(kitty=kitty, status=status):
                self.child(kitty=kitty, status=status)
                popup = self.popup(kitty=kitty)
                master, slave = self.fds[-2:]
                drain = pty_keymaps.drain
                read = os.read
                output = bytearray()

                def check_read(fd, size):
                    if fd == master:
                        os.fstat(slave)
                    chunk = read(fd, size)
                    if fd == master:
                        output.extend(chunk)
                    return chunk

                def check_drain(fd):
                    self.assertIsNotNone(popup.child.returncode)
                    try:
                        os.fstat(slave)
                    except OSError:
                        self.fail("parent slave closed before post-exit tail drain")
                    self.assertEqual(fd, master)
                    tail = drain(fd)
                    self.assertEqual(output, (b"\x1b[<u" if kitty else b"") + b"\x1b[?25h")
                    return tail

                with patch.object(pty_keymaps.os, "read", side_effect=check_read), patch.object(pty_keymaps, "drain", side_effect=check_drain):
                    popup.finish(b"q", success=status == 0)
                self.assert_closed()

    def test_cleanup_reaps_child_and_does_not_close_reused_fds(self):
        for finish in (False, True):
            self.child()
            popup = self.popup()
            if finish:
                popup.finish(b"q")
            else:
                popup.abort()
            self.assertIsNotNone(popup.child.returncode)
            self.assert_closed()
            reader, writer = os.pipe()
            try:
                popup.abort()
                os.write(writer, b"alive")
                self.assertEqual(os.read(reader, 5), b"alive")
            finally:
                os.close(reader)
                os.close(writer)

    def test_finish_drains_repaint_larger_than_pty_queue_before_exit(self):
        for kitty in (False, True):
            with self.subTest(kitty=kitty):
                self.child(kitty=kitty, repaint=True)
                popup = self.popup(kitty=kitty)
                popup.finish(b'\x1b\x1b')
                self.assertEqual(popup.child.returncode, 0)
                self.assert_closed()

    def test_finish_timeout_reaps_child_and_closes_both_fds(self):
        self.child(linger=True)
        popup = self.popup()
        with self.assertRaises(subprocess.TimeoutExpired) as failure:
            popup.finish(b"q")
        self.assertEqual(failure.exception.output, b"")
        self.assertIn("kitty=False key=b'q' received=0", failure.exception.__notes__[0])
        self.assertIn("termios=", failure.exception.__notes__[0])
        self.assertIsNotNone(popup.child.returncode)
        self.assert_closed()

    def test_finish_unexpected_status_closes_both_fds(self):
        self.child(status=7)
        popup = self.popup()
        with self.assertRaises(AssertionError):
            popup.finish(b"q")
        self.assert_closed()

    def test_invalid_dimensions_close_both_fds(self):
        with self.assertRaises(struct.error):
            self.popup(width=65536)
        self.assert_closed()

    def test_finish_assertion_closes_both_fds(self):
        self.child(restore=False)
        popup = self.popup()
        with self.assertRaises(AssertionError):
            popup.finish(b"q")
        self.assertIsNotNone(popup.child.returncode)
        self.assert_closed()

    def test_initial_frame_failure_reaps_child_and_closes_both_fds(self):
        self.child(kitty=True, initial=False)
        children = []
        popen = subprocess.Popen

        def record_child(*args, **kwargs):
            child = popen(*args, **kwargs)
            children.append(child)
            return child

        with patch.object(pty_keymaps.subprocess, "Popen", side_effect=record_child):
            with self.assertRaises(AssertionError):
                self.popup(kitty=True)
        self.assertIsNotNone(children[0].returncode)
        self.assert_closed()

    def test_spawn_failure_closes_both_fds(self):
        with self.assertRaises(FileNotFoundError):
            self.popup()
        self.assert_closed()


if __name__ == "__main__":
    unittest.main()
