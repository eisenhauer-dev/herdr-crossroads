"""Closed-pipe EOF and real-PTY lifecycle regressions."""
import errno
import os
import pty
import socket
import struct
import subprocess
import sys
import termios
import tempfile
import threading
import tty
import unittest
from pathlib import Path
from unittest.mock import patch

import pty_keymaps
import pty_render


class TerminalRestorationDiagnosticsTests(unittest.TestCase):
    def test_equal_attributes_are_checked_once_without_reading_output(self):
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        self.addCleanup(os.close, slave)
        saved = termios.tcgetattr(slave)
        with patch.object(pty_render.termios, "tcgetattr", return_value=saved) as attributes, \
             patch.object(pty_render.select, "select") as ready:
            pty_render.assert_terminal_restored(master, slave, saved, saved, b"q", "cursor")
        attributes.assert_called_once_with(slave)
        ready.assert_not_called()

    def test_every_attribute_mismatch_retains_exact_equality_and_diagnostics(self):
        saved = [1, 2, 3, 4, 9600, 9600, [b"\x03", b"\x1c"]]
        active = [0, 0, 3, 0, 9600, 9600, [b"\x00", b"\x00"]]
        for index, name in enumerate(("iflag", "oflag", "cflag", "lflag", "ispeed", "ospeed", "cc")):
            for ending in (b"", OSError(errno.EIO, "PTY closed")):
                with self.subTest(attribute=name, ending=ending):
                    actual = saved.copy()
                    actual[index] = [b"\x04", b"\x1c"] if name == "cc" else actual[index] ^ 1
                    with patch.object(pty_render.termios, "tcgetattr", return_value=actual) as attributes, \
                         patch.object(pty_render.select, "select", return_value=([123], [], [])) as ready, \
                         patch.object(pty_render.os, "read", side_effect=[b"stty: restore error\r\n", ending]), \
                         self.assertRaises(AssertionError) as failure:
                        pty_render.assert_terminal_restored(123, 456, saved, active, b"\x1b", "notice\x1b[?25h")
                    attributes.assert_called_once_with(456)
                    self.assertTrue(all(call.args[-1] == 0 for call in ready.call_args_list))
                    message = str(failure.exception)
                    for expected in ("key=b'\\x1b'", f"'{name}':", f"saved={saved!r}",
                                     f"active={active!r}", f"actual={actual!r}", "platform=", "python=",
                                     "notice\\x1b[?25h", "stty: restore error\\r\\n"):
                        self.assertIn(expected, message)


class SearchCursorFrameTests(unittest.TestCase):
    @staticmethod
    def draw(width, mode, text, cursor_x):
        row = "  Search: " + text
        row += " " * (width - 2 - len(mode) - sum(2 if c == "界" else 1 for c in row)) + mode
        return f"\x1b[H\x1b[J{row}\x1b[1;{cursor_x + 1}H\x1b[?25h".encode()

    def run_cases(self, delivery, times_out=False, pending=b"", parsing_cost=False):
        now = 0.0
        emissions = []
        popups = []
        test = self
        screen = pty_render.screen

        def parse(raw, *args, **kwargs):
            nonlocal now
            if parsing_cost:
                # Charge per repaint interpreted, not per host CPU speed.
                now += 0.001 * raw.count("\x1b[H\x1b[J")
            return screen(raw, *args, **kwargs)

        class Popup:
            def __init__(self, binary, directory, width, height):
                self.master, self.width, self.height = 123, width, height
                self.output = pty_keymaps.DrawReader(self.master)
                self.mode = "Search"
                popups.append(self)

            def send(self, data):
                if data == b"?":
                    self.mode = "Help"

            def finish(self, key):
                test.assertFalse(emissions, "final query draw was not consumed")
                test.assertEqual(bytes(self.output.pending), pending if self.width == 142 and self.mode == "Search" and self.query == "x" * 200 else b"")

            def abort(self):
                pass

        def write(fd, data):
            popup = popups[-1]
            query = "" if data == b"\x7f" else data.decode()
            popup.query = query
            budget = popup.width - 4 - len(popup.mode) - 1
            if len(query) < 10:
                text, x = query, 10 + len(query)
            elif query.startswith("界"):
                count = (budget - 10) // 2
                text, x = "界" * count + "…", 11 + 2 * count
            else:
                text, x = "x" * (budget - 10) + "…", budget + 1
            final = self.draw(popup.width, popup.mode, text, x)
            if popup.width == 142 and popup.mode == "Search" and query == "x" * 200:
                intermediate = self.draw(142, "Search", "x" * 104, 114)
                emissions.extend(delivery(now, intermediate, final))
            else:
                emissions.append((now, final))
            return len(data)

        def ready(readers, writers, errors, timeout):
            nonlocal now
            self.assertGreater(timeout, 0)
            self.assertLessEqual(timeout, 3)
            self.assertNotIn(timeout, (0.01, 0.05), "query synchronization must not wait for an output gap")
            if emissions and emissions[0][0] <= now + timeout:
                now = max(now, emissions[0][0])
                return readers, [], []
            now += timeout
            return [], [], []

        def read(fd, size):
            at, payload = emissions.pop(0)
            self.assertLessEqual(at, now)
            self.assertLessEqual(len(payload), size)
            return payload

        with patch.object(pty_keymaps, "Popup", Popup), \
             patch.object(pty_render, "screen", side_effect=parse), \
             patch.object(pty_keymaps.time, "monotonic", side_effect=lambda: now), \
             patch.object(pty_render.select, "select", side_effect=ready), \
             patch.object(pty_render.os, "read", side_effect=read), \
             patch.object(pty_render.os, "write", side_effect=write):
            if times_out:
                with self.assertRaisesRegex(AssertionError, "bytearray"):
                    pty_render.search_cursor_cases("unused")
                self.assertEqual(now, 3.0, "intermediate draws extended the original deadline")
            else:
                pty_render.search_cursor_cases("unused")

    def test_intermediate_complete_query_draw_is_not_final(self):
        for delivery in (
            lambda now, intermediate, final: [(now, intermediate), (now + 0.1, final)],
            lambda now, intermediate, final: [(now, intermediate + final[:-1]), (now + 0.1, final[-1:])],
        ):
            with self.subTest(delivery=delivery):
                self.run_cases(delivery)

    def test_correct_cursor_with_wrong_query_is_not_final(self):
        self.run_cases(lambda now, intermediate, final: [
            (now, final.replace(b"xxxxx", b"yyyyy", 1)), (now + 0.1, final),
        ])

    def test_matching_query_requires_complete_cursor_marker(self):
        self.run_cases(lambda now, intermediate, final: [(now, final[:-1])], times_out=True)

    def test_wrong_query_or_cursor_cannot_satisfy_final_state(self):
        for invalid in (
            lambda final: final.replace(b"xxxxx", b"yyyyy", 1),
            lambda final: final.replace(b"\x1b[1;133H", b"\x1b[1;132H"),
        ):
            with self.subTest(invalid=invalid):
                self.run_cases(lambda now, intermediate, final: [(now, invalid(final))], times_out=True)

    def test_matching_frame_retains_read_ahead(self):
        tail = b"next draw\x1b[?25l"
        self.run_cases(lambda now, intermediate, final: [(now, intermediate + final + tail)], pending=tail)

    def test_final_query_must_arrive_within_original_deadline(self):
        self.run_cases(lambda now, intermediate, final: [(now, intermediate), (now + 2.99, final)])
        for final_at in (3.01, 3.1):
            with self.subTest(final_at=final_at):
                self.run_cases(lambda now, intermediate, final: [
                    (now, intermediate), (now + 1, intermediate), (now + 2, intermediate),
                    (now + final_at, final),
                ], times_out=True)

    def test_intermediate_repaints_are_not_replayed_quadratically(self):
        self.run_cases(lambda now, intermediate, final: [
            (now, intermediate * 100), (now + 2.5, final),
        ], parsing_cost=True)


class ReaderEofTests(unittest.TestCase):
    def check_reader(self, expression, payload=b"", expected=None, error=None):
        code = f"""
import os
import sys
import time
sys.dont_write_bytecode = True
sys.path.insert(0, {str(Path(__file__).resolve().parent)!r})
from pty_render import frame
from pty_keymaps import DrawReader, read_until, read_frame, drain, read_search_escape
reader, writer = os.pipe()
os.write(writer, {payload!r})
os.close(writer)
output = DrawReader(reader)
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
        self.check_reader("read_frame(output)", b"frame\x1b[?25h", "frame\x1b[?25h")

    def test_keymap_expected_frame_eof(self):
        self.check_reader("read_frame(output, expected=lambda raw: False)", error="EOF")

    def test_escape_frame_eof(self):
        self.check_reader("read_search_escape(output, time.monotonic(), 'codex', [])", b"\x1b[H", error="EOF")

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

    def observe(self, emissions, help_search=False):
        # Emission times are receive times, independent of host/thread scheduling.
        received = iter((at, payload) for at, payload in emissions if payload)
        pending = next(received, None)
        now = 0.0

        def ready(readers, writers, errors, timeout):
            nonlocal now
            if pending is not None and pending[0] <= now + timeout:
                now = max(now, pending[0])
                return readers, [], []
            now += timeout
            return [], [], []

        def read(fd, size):
            nonlocal pending
            self.assertIsNotNone(pending, "read without a scheduled emission")
            at, payload = pending
            self.assertLessEqual(at, now)
            pending = (at, payload[size:]) if len(payload) > size else next(received, None)
            return payload[:size]

        lines, _ = pty_keymaps.screen(self.frame(help_search=help_search).decode(), 80, 15)
        with patch.object(pty_keymaps.time, "monotonic", side_effect=lambda: now), \
             patch.object(pty_keymaps.select, "select", side_effect=ready), \
             patch.object(pty_keymaps.os, "read", side_effect=read):
            return pty_keymaps.read_search_escape(pty_keymaps.DrawReader(123), 0.0, "codex", lines)

    def test_delayed_search_repaint_is_not_escape_resolution(self):
        raw = self.observe([(0.025, self.frame()), (0.105, self.frame(False))])
        self.assertEqual(raw, self.frame(False).decode())

    def test_complete_draws_remain_separate_on_real_socket_and_pty(self):
        for real_pty in (False, True):
            with self.subTest(real_pty=real_pty):
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
                os.write(reader, b"\x1b")
                self.assertEqual(os.read(writer, 1), b"\x1b")
                os.write(writer, self.frame() + self.frame(False))
                output = pty_keymaps.DrawReader(reader)
                self.assertEqual(pty_keymaps.read_frame(output, expected=lambda raw: True), self.frame().decode())
                self.assertEqual(pty_keymaps.read_frame(output, expected=lambda raw: True), self.frame(False).decode())

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

    def test_fixture_overhead_cannot_make_early_output_timely(self):
        for delayed in ("thread", "screen"):
            with self.subTest(delayed=delayed):
                now = 0.0
                start_thread = threading.Thread.start
                screen = pty_keymaps.screen

                def start(worker):
                    nonlocal now
                    if delayed == "thread":
                        now = 0.105
                    return start_thread(worker)

                def prepare(*args, **kwargs):
                    nonlocal now
                    if delayed == "screen":
                        now = 0.105
                    return screen(*args, **kwargs)

                with patch.object(pty_keymaps.time, "monotonic", side_effect=lambda: now), \
                     patch.object(threading.Thread, "start", start), \
                     patch.object(pty_keymaps, "screen", side_effect=prepare), \
                     self.assertRaisesRegex(AssertionError, "Esc resolved before its deadline"):
                    self.observe([(0.025, self.frame(False))])

    def test_fragmented_search_and_cursor_marker_cross_guard_boundary(self):
        search = self.frame()
        raw = self.observe([(0.025, search[:-1]), (0.08, search[-1:]), (0.105, self.frame(False))])
        self.assertEqual(raw, self.frame(False).decode())

    def test_early_select_body_with_late_cursor_marker_is_rejected(self):
        for help_search, repaint in ((False, b""), (False, self.frame()), (True, self.frame(help_search=True))):
            with self.subTest(help_search=help_search, search_repaint=bool(repaint)):
                navigation = self.frame(False, help_search=help_search)
                emissions = [(0.01, repaint), (0.025, navigation[:-1]), (0.105, navigation[-1:])]
                received = [(at, bytes([byte])) for at, payload in emissions for byte in payload]
                with self.assertRaisesRegex(AssertionError, "Esc resolved before its deadline"):
                    self.observe(received, help_search=help_search)

    def test_fast_send_waits_for_matching_complete_selection_without_quiet_gap(self):
        reader, writer = socket.socketpair()
        self.addCleanup(reader.close)
        self.addCleanup(writer.close)
        popup = pty_keymaps.Popup.__new__(pty_keymaps.Popup)
        popup.master, popup.width, popup.height = reader.fileno(), 80, 15
        popup.output = pty_keymaps.DrawReader(popup.master)
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

    def test_expected_frame_batches_ready_bytes_without_consuming_the_next_draw(self):
        reader, writer = socket.socketpair()
        self.addCleanup(reader.close)
        self.addCleanup(writer.close)
        first, second = self.frame(), self.frame(False)
        output = pty_keymaps.DrawReader(reader.fileno())
        writer.sendall(first + second)
        read = os.read
        calls = []

        def counted_read(fd, size):
            calls.append(size)
            return read(fd, size)

        with patch.object(pty_keymaps.os, "read", side_effect=counted_read):
            raw = pty_keymaps.read_frame(output, expected=lambda raw: True)
        self.assertEqual(raw, first.decode())
        self.assertLessEqual(len(calls), 2, "ready frame bytes must not require per-byte system calls")
        self.assertEqual(pty_keymaps.read_frame(output, expected=lambda raw: True), second.decode())

    def test_buffered_early_navigation_cannot_gain_a_late_receive_timestamp(self):
        reader, writer = socket.socketpair()
        self.addCleanup(reader.close)
        self.addCleanup(writer.close)
        output = pty_keymaps.DrawReader(reader.fileno())
        writer.sendall(self.frame() + self.frame(False))
        start = 0.0
        with patch.object(pty_keymaps.time, "monotonic", return_value=start):
            pty_keymaps.read_frame(output, expected=lambda raw: True)
        lines, _ = pty_keymaps.screen(self.frame().decode(), 80, 15)
        # Processing a previous draw cannot make read-ahead navigation look timely.
        with patch.object(pty_keymaps.time, "monotonic", return_value=start + 0.105), \
             self.assertRaisesRegex(AssertionError, "Esc resolved before its deadline"):
            pty_keymaps.read_search_escape(output, start, "codex", lines)

    def test_complete_sequence_delayed_by_timeout_is_not_observed_as_immediate(self):
        popup = pty_keymaps.Popup.__new__(pty_keymaps.Popup)
        popup.master, popup.width, popup.height = 123, 80, 15
        popup.output = pty_keymaps.DrawReader(popup.master)
        final = self.frame(False)
        now = 0.0

        def ready(readers, writers, errors, timeout):
            nonlocal now
            self.assertGreaterEqual(timeout, 0.100)
            now += 0.100
            return readers, [], []

        with patch.object(pty_keymaps.time, "monotonic", side_effect=lambda: now), \
             patch.object(pty_keymaps.select, "select", side_effect=ready), \
             patch.object(pty_keymaps.os, "read", return_value=final), \
             patch.object(pty_keymaps.os, "write", return_value=4) as write:
            start = pty_keymaps.time.monotonic()
            popup.send(b"\x1b\x1b[A", expected=lambda raw: raw == final.decode())
            elapsed = pty_keymaps.time.monotonic() - start
            self.assertGreaterEqual(elapsed, 0.095, "a 100-ms wait must still fail the fast-input bound")
        write.assert_called_once_with(123, b"\x1b\x1b[A")

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

    def test_transition_timing_boundaries_are_not_relaxed(self):
        for at in (0.09, 0.219):
            with self.subTest(at=at):
                self.assertEqual(self.observe([(at, self.frame(False))]), self.frame(False).decode())
        for at in (0.075, 0.089, 0.22):
            with self.subTest(at=at), self.assertRaisesRegex(AssertionError, "Esc transition timing"):
                self.observe([(at, self.frame(False))])


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
