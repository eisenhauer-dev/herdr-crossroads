"""Closed-pipe EOF and real-PTY lifecycle regressions."""
import contextlib
import copy
import errno
import json
import os
import pty
import signal
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
from unittest.mock import Mock, patch

import pty_keymaps
import pty_render

DARWIN_PENDIN = 0x20000000


class ScreenColorTests(unittest.TestCase):
    def test_panel_background_uses_effective_colors_through_reverse_and_reset(self):
        raw = "\x1b[48;2;24;24;37m\x1b[38;2;205;214;244mA\x1b[7mB\x1b[27mC\x1b[7mD\x1b[0mE"
        lines, wraps, backgrounds = pty_render.screen(raw, 5, 1, track_panel_bg=True)
        self.assertEqual((lines, wraps), (["ABCDE"], 0))
        self.assertEqual(backgrounds, [[True, False, True, False, False]])

    def test_reverse_swaps_indexed_selection_and_panel_foreground(self):
        raw = "\x1b[48;5;236m\x1b[38;2;24;24;37mA\x1b[7mB\x1b[27mC"
        self.assertEqual(pty_render.screen(raw, 3, 1, track_bg=True)[2], [[True, False, True]])
        self.assertEqual(pty_render.screen(raw, 3, 1, track_panel_bg=True)[2], [[False, True, False]])

    def test_effective_colors_keep_defaults_ansi_rgb_and_indexed_roles(self):
        raw = "A\x1b[49;7mB\x1b[7mC\x1b[27mD\x1b[34m\x1b[48;5;236mE\x1b[7mF\x1b[39;49mG\x1b[0mH"
        colors = pty_render.screen(raw, 8, 1, track_colors=True)[2][0]
        normal = ("default-fg", "default-bg")
        self.assertEqual(colors, [normal, normal[::-1], normal[::-1], normal,
                                  (4, 236), (236, 4), normal[::-1], normal])
        raw = "\x1b[38;2;76;79;105m\x1b[48;2;239;241;245m\x1b[7mA\x1b[27mB"
        self.assertEqual(pty_render.screen(raw, 2, 1, track_colors=True)[2],
                         [[((239, 241, 245), (76, 79, 105)), ((76, 79, 105), (239, 241, 245))]])

    def test_cursor_and_erase_do_not_reset_reverse_or_colors(self):
        raw = "\x1b[48;2;24;24;37m\x1b[38;2;205;214;244m\x1b[7m\x1b[H\x1b[J\x1b[1;2HA\x1b[K"
        colors = pty_render.screen(raw, 4, 1, track_colors=True)[2]
        self.assertEqual(colors, [[((24, 24, 37), (205, 214, 244))] * 4])

    def test_panel_assertion_rejects_reverse_leaking_past_mode_badge(self):
        badge = "MODE: SELECT"
        row = pty_render.expected_search_row(80, "f to search", "Select")
        prefix = "\x1b[H\x1b[J\x1b[48;2;24;24;37m\x1b[38;2;205;214;244m"
        raw = prefix + row.replace(badge, f"\x1b[7m{badge}\x1b[27m") + "\x1b[K\r\n\x1b[K"
        pty_render.assert_panel_background(raw, 80, 2)
        with self.assertRaises(AssertionError):
            pty_render.assert_panel_background(raw.replace("\x1b[27m", ""), 80, 2)
        for invalid in (raw.replace("\x1b[7m", ""),
                        raw.replace("\x1b[7m", "\x1b[38;2;1;2;3m\x1b[7m"),
                        raw.replace(" \x1b[7m", "\x1b[7m ")):
            with self.subTest(raw=invalid), self.assertRaises(AssertionError):
                pty_render.assert_panel_background(invalid, 80, 2)


class TerminalRestorationDiagnosticsTests(unittest.TestCase):
    def setUp(self):
        pendin = patch.object(termios, "PENDIN", DARWIN_PENDIN, create=True)
        pendin.start()
        self.addCleanup(pendin.stop)

    def test_equal_attributes_are_checked_once_without_reading_output(self):
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        self.addCleanup(os.close, slave)
        saved = termios.tcgetattr(slave)
        with patch.object(pty_render.termios, "tcgetattr", return_value=saved) as attributes, \
             patch.object(pty_render.select, "select") as ready, \
             patch("platform.mac_ver") as version:
            pty_render.assert_terminal_restored(master, slave, saved, saved, b"q", "cursor",
                                                pre_close=[0], cursor_marker_observed=[0])
        attributes.assert_called_once_with(slave)
        ready.assert_not_called()
        version.assert_not_called()

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
                        pty_render.assert_terminal_restored(123, 456, saved, active, b"\x1b", "notice\x1b[?25h",
                                                            pre_close=active, cursor_marker_observed=saved)
                    attributes.assert_called_once_with(456)
                    self.assertTrue(all(call.args[-1] == 0 for call in ready.call_args_list))
                    message = str(failure.exception)
                    for expected in ("key=b'\\x1b'", f"'{name}':", f"saved={saved!r}",
                                     f"active={active!r}", f"actual={actual!r}", "platform=", "python=",
                                     "notice\\x1b[?25h", "stty: restore error\\r\\n", f"pre_close={active!r}",
                                     f"cursor_marker_observed={saved!r}", f"post_exit={actual!r}"):
                        self.assertIn(expected, message)

    def test_darwin_failure_reports_observation_stages_and_versions(self):
        saved = [1, 2, 3, 1483, 9600, 9600, [b"\x03"]]
        actual = saved.copy()
        actual[3] = 536872395
        actual[1] ^= 1
        with patch.object(pty_render.sys, "platform", "darwin"), \
             patch("platform.mac_ver", return_value=("15.0", ("", "", ""), "arm64")), \
             patch.object(pty_render.termios, "tcgetattr", return_value=actual) as attributes, \
             patch.object(pty_render.select, "select", return_value=([], [], [])), \
             self.assertRaises(AssertionError) as failure:
            pty_render.assert_terminal_restored(123, 456, saved, saved, b"q", "cursor")
        attributes.assert_called_once_with(456)
        for expected in ("pre_close=None", "cursor_marker_observed=None", f"post_exit={actual!r}",
                         "macos='15.0'", "darwin_kernel=", "python=", "sdk=unknown"):
            self.assertIn(expected, str(failure.exception))

    def test_darwin_canonical_added_pendin_is_accepted_without_reading_or_mutation(self):
        saved = [1, 2, 3, 1483, 9600, 9600, [b"\x03"]]
        active = [0, 0, 3, 0, 9600, 9600, [b"\x00"]]
        actual = saved.copy()
        actual[3] = 536872395
        for machine in ("arm64", "x86_64"):
            for key in (b"\r", b"\x1b", b"q"):
                for marker in (active, actual):
                    observations = copy.deepcopy((saved, actual, active, marker))
                    with self.subTest(machine=machine, key=key, marker=marker), \
                         patch.object(pty_render.sys, "platform", "darwin"), \
                         patch("platform.mac_ver", return_value=("15.0", ("", "", ""), machine)), \
                         patch.object(pty_render.os, "uname", return_value=os.uname_result(("Darwin", "host", "24.0.0", "version", machine))), \
                         patch.object(pty_render.termios, "tcgetattr", return_value=actual) as attributes, \
                         patch.object(pty_render.select, "select", return_value=([], [], [])) as ready, \
                         patch.object(pty_render.os, "read") as read:
                        pty_render.assert_terminal_restored(123, 456, saved, active, key, "cursor",
                                                            pre_close=active, cursor_marker_observed=marker)
                    attributes.assert_called_once_with(456)
                    ready.assert_not_called()
                    read.assert_not_called()
                    self.assertEqual((saved, actual, active, marker), observations)

    def test_restoration_rejects_every_change_outside_the_narrow_darwin_exception(self):
        saved = [1, 2, 3, 1483, 9600, 9600, [b"\x03", b"\x1c"]]
        cases = []
        for index in range(7):
            actual = saved.copy()
            actual[3] |= DARWIN_PENDIN
            actual[index] = [b"\x04", b"\x1c"] if index == 6 else actual[index] ^ 1
            cases.append(("darwin", saved, actual))
        pending = saved.copy()
        pending[3] |= DARWIN_PENDIN
        noncanonical = saved.copy()
        noncanonical[3] &= ~termios.ICANON
        noncanonical_pending = noncanonical.copy()
        noncanonical_pending[3] |= DARWIN_PENDIN
        cases.extend([(platform, saved, pending) for platform in ("linux", "freebsd", "win32")])
        cases.extend([("darwin", pending, saved), ("darwin", noncanonical, noncanonical_pending)])
        for platform, expected, actual in cases:
            with self.subTest(platform=platform, expected=expected, actual=actual), \
                 patch.object(pty_render.sys, "platform", platform), \
                 patch.object(pty_render.termios, "tcgetattr", return_value=actual), \
                 patch.object(pty_render.select, "select", return_value=([], [], [])), \
                 self.assertRaises(AssertionError):
                pty_render.assert_terminal_restored(123, 456, expected, [0] * 7, b"q", "cursor")

    def test_exact_equality_including_original_pendin_is_accepted_on_every_platform(self):
        for platform in ("darwin", "linux", "freebsd", "win32"):
            for lflag in (1483, 1483 | DARWIN_PENDIN, 0, DARWIN_PENDIN):
                saved = [1, 2, 3, lflag, 9600, 9600, [b"\x03"]]
                with self.subTest(platform=platform, lflag=lflag), \
                     patch.object(pty_render.sys, "platform", platform), \
                     patch.object(pty_render.termios, "tcgetattr", return_value=saved), \
                     patch.object(pty_render.select, "select") as ready:
                    pty_render.assert_terminal_restored(123, 456, saved, [0] * 7, b"q", "cursor")
                ready.assert_not_called()

    def test_popup_finish_uses_the_same_restoration_contract_and_keeps_cleanup_modes(self):
        for kitty in (False, True):
            for changed in (False, True):
                saved = [1, 2, 3, 1483, 9600, 9600, [b"\x03"]]
                actual = saved.copy()
                actual[3] |= DARWIN_PENDIN
                if changed:
                    actual[2] ^= 1
                popup = pty_keymaps.Popup.__new__(pty_keymaps.Popup)
                popup.master, popup.slave = pty.openpty()
                fds = (popup.master, popup.slave)
                popup.kitty = kitty
                popup.saved_termios = saved
                popup.child = Mock(returncode=0)
                popup.child.poll.return_value = 0
                popup.output = pty_keymaps.DrawReader(popup.master)
                popup.output.pending = bytearray(b"\x1b[?1003l\x1b[?1006l\x1b[?25h" + (b"\x1b[<u" if kitty else b""))
                with self.subTest(kitty=kitty, additional_change=changed), \
                     patch.object(pty_keymaps.sys, "platform", "darwin"), \
                     patch.object(pty_keymaps.termios, "tcgetattr", return_value=actual) as attributes, \
                     patch.object(pty_keymaps, "drain", return_value=b""), \
                     (self.assertRaisesRegex(AssertionError, "terminal restoration failed") if changed else contextlib.nullcontext()):
                    popup.finish(b"q", expect_mouse=True)
                attributes.assert_called_once_with(fds[1])
                for fd in fds:
                    with self.assertRaises(OSError):
                        os.fstat(fd)


class ModeColorFrameTests(unittest.TestCase):
    def run_cases(self, delivery="delayed", missing=False, invalid_colors=False, read_ahead=False):
        now = 0.0
        emissions = []
        popups = []
        test = self
        tail = b"next draw\x1b[?25l" if read_ahead else b""
        transitions = (
            (b"f", "SELECT", 0, "", True), (b"\r", "SELECT", 0, "f to search", False),
            (b"?", "HELP", 0, "f to search", False), (b"f", "HELP", 0, "", True),
            (b"\r", "HELP", 0, "f to search", False), (b"q", "SELECT", 0, "f to search", False),
            (b"t", None, 0, "", False), (b" ", "THEMES", 1, "", False),
            (pty_keymaps.mouse_report(4, 1), "THEMES", 0, "", True),
            (b"x", "THEMES", 0, "x", True), (b"\x7f", "THEMES", 0, "", True),
            (b"\r", "THEMES", 1, "", False),
            (b"?", "HELP", 0, "f to search", False), (b"f", "HELP", 0, "", True),
            (b"\r", "HELP", 0, "f to search", False), (b"q", "THEMES", 1, "", False),
            (b"q", "SELECT", 0, "f to search", False),
        )

        def paint(popup, mode="SELECT", y=0, query="f to search", searching=False):
            raw = "\x1b[H\x1b[J" + popup.style
            if mode is None:
                raw += "  Themes · selected: catppuccin · preview: catppuccin · keys"
            else:
                row = pty_render.expected_search_row(popup.width, query, mode)
                badge = f"MODE: {mode}"
                if y:
                    raw += "  Themes · saved: catppuccin · preview: catppuccin\r\n"
                raw += row.replace(badge, f"\x1b[7m{badge}\x1b[27m") + "\x1b[K"
            raw += f"\x1b[1;{11 + len(query)}H\x1b[?25h" if searching else "\x1b[?25l"
            return raw.encode()

        class Popup:
            def __init__(self, binary, directory, width, height):
                nonlocal now
                now = 0.0
                theme = json.loads((directory / "theme.jsonc").read_text())["theme"]
                self.style = {
                    "catppuccin": "\x1b[38;2;205;214;244m\x1b[48;2;24;24;37m",
                    "catppuccin-latte": "\x1b[38;2;76;79;105m\x1b[48;2;239;241;245m",
                    "terminal": "\x1b[39;49m",
                }[theme]
                self.master, self.width, self.height = 123, width, height
                self.output = pty_keymaps.DrawReader(self.master)
                self.previous = paint(self)
                self.initial = self.previous.decode()
                self.step = 0
                popups.append(self)

            def finish(self, key):
                test.assertFalse(emissions)
                test.assertEqual(bytes(self.output.pending), tail)
                test.assertEqual(self.step, len(transitions))

            def abort(self):
                pass

        def write(fd, key):
            popup = popups[-1]
            self.assertFalse(emissions, "next input sent before matching mode/query/cursor draw")
            self.assertEqual(bytes(popup.output.pending), b"")
            expected_key, *state = transitions[popup.step]
            self.assertEqual(key, expected_key)
            final = paint(popup, *state)
            popup.step += 1
            if invalid_colors:
                final = final.replace(b"\x1b[7m", b"")
            wrong_cursor = final[:-1] + (b"l" if final.endswith(b"h") else b"h")
            if delivery == "coalesced":
                emissions.append((now, popup.previous + wrong_cursor + final + (tail if popup.step == len(transitions) else b"")))
            else:
                emissions.append((now, popup.previous + wrong_cursor))
                if missing:
                    emissions.extend([(now + 1, popup.previous), (now + 2, popup.previous), (now + 3.01, final)])
                elif delivery == "fragmented":
                    emissions.extend([(now + 0.02, final[:-1]), (now + 0.1, final[-1:])])
                else:
                    emissions.append((now + 2.99, final))
            popup.previous = final
            return len(key)

        def ready(readers, writers, errors, timeout):
            nonlocal now
            if emissions and emissions[0][0] <= now + timeout:
                now = max(now, emissions[0][0])
                return readers, [], []
            now += timeout
            return [], [], []

        with patch.object(pty_keymaps, "Popup", Popup), \
             patch.object(pty_keymaps.time, "monotonic", side_effect=lambda: now), \
             patch.object(pty_keymaps.select, "select", side_effect=ready), \
             patch.object(pty_keymaps.os, "read", side_effect=lambda fd, size: emissions.pop(0)[1]), \
             patch.object(pty_keymaps.os, "write", side_effect=write):
            if missing:
                with self.assertRaises(AssertionError):
                    pty_render.mode_color_cases("unused")
                self.assertEqual(now, 3.0, "stale draws must not shorten or extend the deadline")
            elif invalid_colors:
                with self.assertRaises(AssertionError):
                    pty_render.mode_color_cases("unused")
            else:
                pty_render.mode_color_cases("unused")

    def test_mode_colors_wait_for_matching_complete_mode_query_and_cursor(self):
        for delivery in ("delayed", "coalesced", "fragmented"):
            with self.subTest(delivery=delivery):
                self.run_cases(delivery)

    def test_stale_mode_frame_cannot_satisfy_transition_before_original_deadline(self):
        self.run_cases(missing=True)

    def test_matching_mode_state_does_not_skip_badge_color_assertions(self):
        self.run_cases(delivery="coalesced", invalid_colors=True)

    def test_matching_mode_frame_preserves_read_ahead(self):
        self.run_cases(delivery="coalesced", read_ahead=True)


class SearchCursorFrameTests(unittest.TestCase):
    @staticmethod
    def draw(width, mode, text, cursor_x):
        mode = f"MODE: {mode.upper()}"
        row = "  Search: " + text
        row += " " * (width - 2 - len(mode) - sum(2 if c == "界" else 1 for c in row)) + mode
        return f"\x1b[H\x1b[J{row}\x1b[1;{cursor_x + 1}H\x1b[?25h".encode()

    def run_cases(self, delivery, times_out=False, pending=b"", parsing_cost=False,
                  target_query="x" * 200, unicode_cost=0):
        now = 0.0
        emissions = []
        popups = []
        test = self
        screen = pty_render.screen
        combining = pty_render.unicodedata.combining

        def character_width(char):
            nonlocal now
            now += unicode_cost
            return combining(char)

        def parse(raw, *args, **kwargs):
            nonlocal now
            if parsing_cost:
                # Charge per repaint interpreted, not per host CPU speed.
                now += 0.001 * raw.count("\x1b[H\x1b[J")
            return screen(raw, *args, **kwargs)

        class Popup:
            def __init__(self, binary, directory, width, height):
                nonlocal now
                now = 0.0  # Each child has its own modeled deadline/clock origin.
                self.master, self.width, self.height = 123, width, height
                self.output = pty_keymaps.DrawReader(self.master)
                self.mode = "Select"
                popups.append(self)

            def send(self, data):
                if data == b"?":
                    self.mode = "Help"
                elif data == b"t":
                    self.mode = "Themes"

            def finish(self, key):
                test.assertFalse(emissions, "final query draw was not consumed")
                test.assertLessEqual(now, 3, "query parsing exceeded the original deadline")
                test.assertEqual(bytes(self.output.pending), pending if self.width == 142 and self.mode == "Select" and self.query == target_query else b"")

            def abort(self):
                pass

        def write(fd, data):
            popup = popups[-1]
            query = "" if data == b"\x7f" else data.decode()
            popup.query = query
            budget = popup.width - 4 - len(f"MODE: {popup.mode.upper()}") - 1
            query_budget = budget - 9
            if len(query) < 10:
                text, x = query, 10 + len(query)
            elif query.startswith("界"):
                count = (query_budget - 1) // 2
                text, x = "界" * count + "Z", 11 + 2 * count
            elif query.startswith("e\u0301"):
                text, x = "e" * (query_budget - 1) + "Z", budget + 1
            elif query.startswith("old"):
                text, x = query[-query_budget:], budget + 1
            else:
                text, x = "x" * query_budget, budget + 1
            final = self.draw(popup.width, popup.mode, text, x)
            if query.startswith("e\u0301"):
                final = final.replace(("e" * (query_budget - 1) + "Z").encode(),
                                      ("e\u0301" * (query_budget - 1) + "Z").encode())
            if popup.width == 142 and popup.mode == "Select" and query == target_query:
                intermediate = (final.replace(b"Z", b"e") if query.startswith("e\u0301")
                                else self.draw(142, "Select", "x" * 104, 114))
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
             patch.object(pty_render.unicodedata, "combining", side_effect=character_width), \
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
            lambda final: final.replace(b"\x1b[1;127H", b"\x1b[1;126H"),
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

    def test_full_unicode_query_repaints_do_not_spend_deadline_on_ascii_widths(self):
        def delivery(now, intermediate, final):
            def full_frame(raw):
                cursor = raw.rfind(b"\x1b[1;")
                body = (b"\r\n" + b" " * 141 + b"\x1b[K") * 29
                return raw[:cursor] + body + raw[cursor:]

            return [(now, full_frame(intermediate)) for _ in range(400)] + [(now, full_frame(final))]

        # Charge two microseconds per Unicode lookup, independently of host CPU.
        # All 401 full-height draws must be consumed before the unchanged 3s limit.
        self.run_cases(delivery, target_query="e\u0301" * 200 + "Z", unicode_cost=0.000002)


class MouseReadinessTests(unittest.TestCase):
    def run_mouse_help(self, delivery="delayed", missing=False, missing_wheel=False):
        now = 0.0
        emissions = []
        sent = []
        panes = ("Claude", "Same", "pane 3", "pane 4", "pane 1", "codex", "pane 2")

        def paint(query="f to search", searching=False, help_view=False, selected=0):
            names = ("pane 3", "pane 4", "pane 1", "pane 2") if query == "pane" else ("codex",) if query != "f to search" else panes
            lines = [pty_keymaps.expected_search_row(120, query, "Help" if help_view else "Select"),
                     "  Keys — Select" if help_view else "  " + "─" * 116,
                     f"    {'Workspace':12} {'Tab':8} {'Agent/Pane Name':24} Status",
                     "  " + "─" * 116]
            lines.extend(f"  {'>' if index == selected else ' '} {'Same':12} {'Same':8} {name:24} idle"
                         for index, name in enumerate(names))
            lines.append("  " + "─" * 116)
            lines.extend([""] * (29 - len(lines)))
            lines.append("  Enter/Esc: select  Backspace: erase" if searching else "  Esc: exit · ?: keys")
            cursor = f"\x1b[1;{11 + len(query)}H\x1b[?25h" if searching else "\x1b[?25l"
            return ("\x1b[?1003h\x1b[?1006h\x1b[H\x1b[J" + "\r\n".join(lines) + cursor).encode()

        popup = pty_keymaps.Popup.__new__(pty_keymaps.Popup)
        popup.master, popup.width, popup.height = 123, 120, 30
        popup.initial = paint().decode()
        popup.output = pty_keymaps.DrawReader(popup.master)
        popup.abort = Mock()

        class HelpObserved(Exception):
            pass

        def write(fd, data):
            sent.append(data)
            if len(sent) == 1:
                emissions.append((now, paint("", searching=True)))
            elif len(sent) == 2:
                self.assertEqual(data, b"pane")
                emissions.append((now, paint("pane", searching=True)))
            elif 3 <= len(sent) <= 6:
                index = len(sent) - 3
                button, selected, previous = [(64, 3, 0), (65, 0, 3), (65, 1, 0), (64, 0, 1)][index]
                self.assertEqual(data, pty_keymaps.mouse_report(5, 5, button=button))
                self.assertEqual(len(emissions), 0, "next wheel sent before matching selection/query/cursor")
                self.assertEqual(popup.output.pending, b"", "stale wheel draw was accepted")
                final = paint("pane", searching=True, selected=selected)
                # The right query/cursor with the old selection, or the right
                # selection with a wrong query/cursor, cannot complete a wheel.
                stale = paint("pane", searching=True, selected=previous)
                wrong_query = final.replace(b"Search: pane", b"Search: panj")
                wrong_cursor = final.replace(b"[1;15H", b"[1;14H")
                emissions.append((now, stale + wrong_query + wrong_cursor))
                if not missing_wheel:
                    emissions.append((now, final))
            elif len(sent) == 7:
                self.assertEqual(data, b"\x7f" * 4)
                self.assertEqual(len(emissions), 0)
                self.assertEqual(popup.output.pending, b"")
                emissions.append((now, paint("", searching=True)))
            elif len(sent) == 8:
                self.assertEqual(data, b"codex")
                # Each key repaints; a gap longer than 10ms is not completion.
                frames = [paint("codex"[:index], searching=True) for index in range(1, 6)]
                if delivery in ("coalesced", "stale Select"):
                    emissions.append((now, b"".join(frames)))
                elif delivery == "fragmented":
                    emissions.extend([(now, b"".join(frames)[:-1]), (now + 0.1, frames[-1][-1:])])
                elif delivery == "wrong cursor":
                    emissions.append((now, frames[-1].replace(b"[1;16H", b"[1;15H")))
                    if not missing:
                        emissions.append((now + 0.1, frames[-1]))
                else:
                    emissions.extend((now + index * 0.02, frame) for index, frame in enumerate(frames, 1))
            elif len(sent) == 9:
                self.assertEqual(len(emissions), 0, "search return sent before the complete query draw")
                # A late editing repaint has the same MODE: SELECT badge.
                emissions.append((now, paint("codex", searching=True) + paint("codex")))
            elif len(sent) == 10:
                self.assertEqual(len(emissions), 0, "Help click sent before search return was observed")
                self.assertEqual(popup.output.pending, b"", "editing repaint was mistaken for search return")
                if delivery == "stale Select":
                    emissions.extend([(now, paint("codex")), (now + 0.02, paint("f to search", help_view=True))])
                else:
                    emissions.append((now, paint("codex") + paint("f to search", help_view=True)))
            else:
                self.assertEqual(popup.output.pending, b"", "pre-Help Select repaint was mistaken for Help")
                raise HelpObserved
            return len(data)

        def ready(readers, writers, errors, timeout):
            nonlocal now
            if emissions and emissions[0][0] <= now + timeout:
                now = max(now, emissions[0][0])
                return readers, [], []
            now += timeout
            return [], [], []

        with patch.object(pty_keymaps, "Popup", return_value=popup), \
             patch.object(pty_keymaps.time, "monotonic", side_effect=lambda: now), \
             patch.object(pty_keymaps.select, "select", side_effect=ready), \
             patch.object(pty_keymaps.os, "read", side_effect=lambda fd, size: emissions.pop(0)[1]), \
             patch.object(pty_keymaps.os, "write", side_effect=write):
            with self.assertRaisesRegex(AssertionError, "bytearray") if missing or missing_wheel else self.assertRaises(HelpObserved):
                pty_keymaps.mouse_cases("unused", Path("unused"))
        self.assertEqual(len(sent), 3 if missing_wheel else 8 if missing else 11)
        if missing or missing_wheel:
            self.assertEqual(now, 3.0, "query repaints extended the original deadline")

    def test_mouse_help_waits_for_typing_and_search_return_before_next_click(self):
        for delivery in ("delayed", "coalesced", "fragmented", "wrong cursor"):
            with self.subTest(delivery=delivery):
                self.run_mouse_help(delivery)

    def test_mouse_query_with_wrong_cursor_cannot_advance_to_search_return(self):
        self.run_mouse_help("wrong cursor", missing=True)

    def test_pre_help_select_repaint_is_not_the_help_click_result(self):
        self.run_mouse_help("stale Select")

    def test_wheel_requires_matching_selection_query_and_editing_cursor(self):
        self.run_mouse_help(missing_wheel=True)


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

    def test_ready_drain_preserves_eof_tail_and_pty_errors(self):
        self.check_reader("drain(reader, timeout=0)", b"tail\x1b[?25h", b"tail\x1b[?25h")
        self.check_reader("drain(reader, timeout=0)", expected=b"")
        for number in (errno.EIO, errno.EBADF):
            with self.subTest(errno=number), \
                 patch.object(pty_keymaps.select, "select", return_value=([123], [], [])), \
                 patch.object(pty_keymaps.os, "read", side_effect=[b"tail", OSError(number, "read failure")]):
                if number == errno.EIO:
                    self.assertEqual(pty_keymaps.drain(123, timeout=0), b"tail")
                else:
                    with self.assertRaises(OSError) as failure:
                        pty_keymaps.drain(123, timeout=0)
                    self.assertEqual(failure.exception.errno, number)

    def test_live_drain_keeps_waiting_for_a_full_quiet_window(self):
        chunks = iter((b"first", b"late"))
        with patch.object(pty_keymaps.select, "select", side_effect=[([123], [], []), ([123], [], []), ([], [], [])]) as ready, \
             patch.object(pty_keymaps.os, "read", side_effect=lambda fd, size: next(chunks)):
            self.assertEqual(pty_keymaps.drain(123), b"firstlate")
        self.assertEqual([call.args[-1] for call in ready.call_args_list], [0.2] * 3)


class EscapeObservationTests(unittest.TestCase):
    @staticmethod
    def frame(searching=True, query="codex", cursor=None, help_search=False):
        header = pty_keymaps.expected_search_row(80, query, "Help" if help_search else "Select")
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

    def child(self, kitty=False, status=0, restore=True, initial=True, linger=False, repaint=False, tail=None, delay=0):
        first_frame = ((b"\x1b[>1u" if kitty else b"") + b"frame\x1b[?25l") if initial else b"bad frame\x1b[?25l"
        if tail is None:
            tail = (b"\x1b[<u" if kitty else b"") + (b"\x1b[?25h" if restore else b"")
        self.binary.write_text(f"""#!{sys.executable}
import os
import time
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
time.sleep({delay!r})
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

                def check_drain(fd, **kwargs):
                    self.assertIsNotNone(popup.child.returncode)
                    try:
                        os.fstat(slave)
                    except OSError:
                        self.fail("parent slave closed before post-exit tail drain")
                    self.assertEqual(fd, master)
                    tail = drain(fd, **kwargs)
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

    def test_completed_child_drains_all_queued_chunks_without_a_quiet_wait(self):
        for kitty in (False, True):
            with self.subTest(kitty=kitty):
                tail = b"queued" * 100 + (b"\x1b[<u" if kitty else b"") + b"trailing diagnostic"
                self.child(kitty=kitty, tail=tail)
                popup = self.popup(kitty=kitty)
                popup.output.pending.extend(b"read ahead\x1b[?25h")
                write, read, select = os.write, os.read, pty_keymaps.select.select
                received = bytearray()

                def exit_before_poll(fd, key):
                    result = write(fd, key)
                    popup.child.wait(timeout=3)
                    return result

                def ready(readers, writers, errors, timeout):
                    self.assertIsNotNone(popup.child.returncode)
                    self.assertEqual(timeout, 0, "completed writer must not incur a quiet wait")
                    os.fstat(popup.slave)
                    return select(readers, writers, errors, timeout)

                def small_read(fd, size):
                    chunk = read(fd, min(size, 7))
                    received.extend(chunk)
                    return chunk

                with patch.object(pty_keymaps.os, "write", side_effect=exit_before_poll), \
                     patch.object(pty_keymaps.os, "read", side_effect=small_read), \
                     patch.object(pty_keymaps.select, "select", side_effect=ready):
                    popup.finish(b"q")
                self.assertEqual(received, tail)
                self.assertEqual(popup.output.pending, b"")
                self.assert_closed()

    def test_late_output_before_exit_is_collected_without_shortening_live_wait(self):
        self.child(kitty=True, delay=0.25)
        popup = self.popup(kitty=True)
        popup.finish(b"q")
        self.assertEqual(popup.child.returncode, 0)
        self.assert_closed()

    def test_finish_rejects_missing_or_duplicate_kitty_pop_with_complete_tail(self):
        for count in (0, 2):
            with self.subTest(pop_count=count):
                tail = b"\x1b[<u" * count + b"\x1b[?25htrailing diagnostic"
                self.child(kitty=True, tail=tail)
                popup = self.popup(kitty=True)
                with self.assertRaises(AssertionError) as failure:
                    popup.finish(b"q")
                self.assertEqual(failure.exception.args, (bytearray(tail),))
                self.assertIsNotNone(popup.child.returncode)
                self.assert_closed()

    def test_stopped_live_child_times_out_and_is_reaped(self):
        self.child()
        popup = self.popup()
        os.kill(popup.child.pid, signal.SIGSTOP)
        with self.assertRaises(subprocess.TimeoutExpired):
            popup.finish(b"q")
        self.assertIsNotNone(popup.child.returncode)
        self.assert_closed()

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
