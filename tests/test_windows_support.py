"""Portable marker/current-screen checks; native ConPTY APIs are not invoked."""
import queue
import ast
from pathlib import Path
import runpy
import unittest
from unittest.mock import Mock, patch


with patch("ctypes.WinDLL", create=True, return_value=Mock()):
    Console = runpy.run_path(str(Path(__file__).with_name("windows_support.py")))["Console"]


class ConsoleMarkerTests(unittest.TestCase):
    def test_native_mouse_flow_observes_selection_and_opt_out_separately(self):
        tree = ast.parse(Path(__file__).with_name("test_windows_acceptance.py").read_text())
        method = next(node for node in ast.walk(tree) if isinstance(node, ast.FunctionDef)
                      and node.name == "test_mouse_search_help_origin_theme_save_wheel_optout_and_cleanup")
        tables = [node for node in ast.walk(method) if isinstance(node, ast.Call)
                  and isinstance(node.func, ast.Attribute) and node.func.attr == "until_table"]
        self.assertGreaterEqual(len(tables), 6, "mouse acceptance needs specific before/after table states, not transcript substrings")
        selections = [ast.literal_eval(keyword.value) for call in tables for keyword in call.keywords
                      if keyword.arg == "selected"]
        self.assertIn((0, "Claude"), selections)
        self.assertIn((1, "Same"), selections)
        self.assertIn((0, "codex"), selections)
        self.assertGreaterEqual(sum(keyword.arg == "capture" and isinstance(keyword.value, ast.Constant)
                                    and keyword.value.value is False
                                    for call in tables for keyword in call.keywords), 3)

    def console(self, *chunks):
        console = Console.__new__(Console)
        console.output = bytearray()
        console.queue = queue.Queue()
        for chunk in (*chunks, b""):
            console.queue.put(chunk)
        return console

    def test_search_highlight_is_one_label_despite_sgr_and_fragmented_reads(self):
        output = (b"\x1b[27X\x1b[38;2;180;190;254m\x1b[4m\x1b[27Ccod"
                  b"\x1b[38;2;205;214;244m\x1b[24mex\x1b[16X")
        self.assertNotIn(b"codex", output)
        for split in range(1, len(output)):
            with self.subTest(split=split):
                console = self.console(output[:split], output[split:])
                result = console.until(b"codex")
                self.assertIn(b"ex", result)
                self.assertEqual(result, bytes(console.output))
                self.assertTrue(output.startswith(result))

    def test_controls_do_not_join_labels_across_cells_lines_or_redraws(self):
        for control in (b"\x1b[1C", b"\x1b[1D", b"\x1b[2J", b"\x1b[H",
                        b"\r", b"\n", b"\t", b"\b", b"\x1b[", b"\x1b[31",
                        b"\x1b[?31m", b"\x1b[31 m", b"\x1b[31\nm"):
            with self.subTest(control=control):
                console = self.console(b"cod" + control, b"ex")
                with self.assertRaisesRegex(AssertionError, "output closed before b'codex'"):
                    console.until(b"codex")

    def test_reset_does_not_join_old_and_new_labels(self):
        console = self.console()
        console.output.extend(b"cod")
        console.reset_output()
        console.queue.put(b"ex")
        console.queue.put(b"")
        with self.assertRaisesRegex(AssertionError, "output closed before b'codex'"):
            console.until(b"codex")

    def test_control_string_payloads_are_not_visible_labels(self):
        for introducer in (b"\x1b]0;", b"\x1bP", b"\x1bX", b"\x1b^", b"\x1b_"):
            for ending in (b"\x07", b"\x1b\\", b""):
                with self.subTest(introducer=introducer, ending=ending):
                    console = self.console(introducer + b"codex", ending or b"\x1b")
                    with self.assertRaisesRegex(AssertionError, "output closed before b'codex'"):
                        console.until(b"codex")

    def test_bel_does_not_expose_non_osc_control_string_payloads(self):
        for introducer in (b"\x1bP", b"\x1bX", b"\x1b^", b"\x1b_"):
            for ending in (b"\x1b\\", b"", b"\x1b"):
                output = introducer + b"q\x07codex" + ending
                for split in range(1, len(output) + 1):
                    with self.subTest(introducer=introducer, ending=ending, split=split):
                        console = self.console(output[:split], output[split:])
                        with self.assertRaisesRegex(AssertionError, "output closed before b'codex'"):
                            console.until(b"codex")

    def test_labels_after_real_control_string_terminators_are_visible(self):
        for introducer, ending in ((b"\x1b]0;", b"\x07"), (b"\x1b]0;", b"\x1b\\"),
                                   (b"\x1bP", b"\x1b\\"), (b"\x1bX", b"\x1b\\"),
                                   (b"\x1b^", b"\x1b\\"), (b"\x1b_", b"\x1b\\")):
            payload = b"q" if introducer == b"\x1b]0;" else b"q\x07codex"
            output = introducer + payload + ending + b"ter\x1b[24mminal"
            for split in range(1, len(output) + 1):
                with self.subTest(introducer=introducer, ending=ending, split=split):
                    console = self.console(output[:split], output[split:])
                    self.assertEqual(console.until(b"terminal"), output)

    def test_plain_and_sgr_labels_work_with_byte_sized_reads(self):
        for output in (b"selected: terminal", b"selected: \x1b[m\x1b[38:2::1:2:3mterminal"):
            with self.subTest(output=output):
                console = self.console(*(bytes([byte]) for byte in output))
                self.assertEqual(console.until(b"selected: terminal"), output)

    def test_raw_protocol_mode_keeps_exact_bytes_and_does_not_ignore_sgr(self):
        output = b"cod\x1b[24mex"
        console = self.console(output)
        self.assertEqual(console.until(b"\x1b[24m", raw=True), output)
        with self.assertRaisesRegex(AssertionError, "output closed before b'codex'"):
            console.until(b"codex", raw=True)

    def test_timeout_preserves_raw_diagnostics_and_default_five_second_deadline(self):
        output = b"cod\x1b[24m"
        for times in ([0, 5], [0, 0]):
            with self.subTest(times=times):
                console = self.console()
                console.output.extend(output)
                console.queue = Mock()
                console.queue.get.side_effect = queue.Empty
                with patch.dict(Console.until.__globals__, time=Mock(monotonic=Mock(side_effect=times))):
                    with self.assertRaises(AssertionError) as caught:
                        console.until(b"codex")
                self.assertIn("missing b'codex'", str(caught.exception))
                self.assertIn(repr(output), str(caught.exception))
                if times[-1] == 0:
                    console.queue.get.assert_called_once_with(timeout=5)
                else:
                    console.queue.get.assert_not_called()

    def test_reader_error_is_not_hidden_by_label_matching(self):
        error = OSError("fixture read failed")
        console = self.console(b"cod\x1b[24m", error)
        with self.assertRaises(OSError) as caught:
            console.until(b"codex")
        self.assertIs(caught.exception, error)


class ConsoleTableTests(unittest.TestCase):
    panes = ("Claude", "Same", "pane 3", "pane 4", "pane 1", "codex", "pane 2")

    def paint(self, *, mode="Select", query="f to search", panes=None, selected=0, capture=False):
        panes = self.panes if panes is None else panes
        left = f"  Search: {query}"
        lines = [left + " " * (118 - len(left) - len(mode)) + mode,
                 "  " + "─" * 116,
                 f"    {'Workspace':12} {'Tab':8} {'Agent/Pane Name':24} Status",
                 "  " + "─" * 116]
        lines.extend(f"  {'>' if index == selected else ' '} {'Same':12} {'Same':8} {name:24} working"
                     for index, name in enumerate(panes))
        lines.append("  " + "─" * 116)
        cursor = f"\x1b[1;{11 + len(query)}H\x1b[?25h" if mode == "Search" else "\x1b[?25l"
        return (f"\x1b[?1003;1006{'h' if capture else 'l'}\x1b[H\x1b[2J" + "\r\n".join(lines) + cursor).encode()

    def console(self, *chunks):
        console = Console.__new__(Console)
        console.output = bytearray()
        console.queue = queue.Queue()
        console.track_screen()
        for chunk in (*chunks, b""):
            console.queue.put(chunk)
        return console

    def selected(self, console, selected):
        return console.until_table(mode="Select", query="f to search", selected=selected, panes=self.panes)

    def test_ignored_wheel_passes_old_marker_but_fails_specific_next_pane(self):
        console = self.console(self.paint(selected=0))
        console.until(b"> ")  # The old assertion accepts an ignored-wheel repaint.
        with self.assertRaisesRegex(AssertionError, "closed before current table"):
            self.selected(console, (1, "Same"))

    def test_selected_pane_follows_cursor_only_native_diffs_across_output_resets(self):
        console = self.console(self.paint(selected=0), b"\x1b[5;3H  \x1b[6;3H> ")
        self.selected(console, (0, "Claude"))
        console.reset_output()
        self.assertEqual(self.selected(console, (1, "Same"))["selected"], (1, "Same"))

    def test_broken_opt_out_wheel_is_detected_before_filter_hides_navigation(self):
        barrier = self.paint(mode="Search", query="", selected=1)
        filtered = self.paint(mode="Search", query="codex", panes=("codex",), selected=0)
        self.console(barrier, filtered).until(b"codex")  # The old compound check passes.
        console = self.console(barrier, filtered)
        with self.assertRaisesRegex(AssertionError, "closed before current table"):
            console.until_table(mode="Search", query="", selected=(0, "Claude"), panes=self.panes, capture=False)

    def test_opt_out_rejects_active_capture_even_with_correct_query_and_result(self):
        raw = self.paint(mode="Search", query="codex", panes=("codex",), capture=True)
        console = self.console(raw)
        console.until(b"codex")
        with self.assertRaisesRegex(AssertionError, "closed before current table"):
            console.until_table(mode="Search", query="codex", selected=(0, "codex"), panes=("codex",), capture=False)

    def test_exact_keyboard_query_and_filtered_rows_reject_substring_and_stale_panes(self):
        for query, panes, selected in [("fcodex", (), None), ("codexq", (), None),
                                       ("codex ", ("codex",), 0),
                                       ("codex", ("codex", "Claude"), 0)]:
            with self.subTest(query=query, panes=panes):
                raw = self.paint(mode="Search", query=query, panes=panes, selected=selected)
                console = self.console(raw)
                console.until(b"codex")
                with self.assertRaisesRegex(AssertionError, "closed before current table"):
                    console.until_table(mode="Search", query="codex", selected=(0, "codex"), panes=("codex",), capture=False)

    def test_exact_keyboard_query_and_single_filtered_result_accept_fragmented_native_output(self):
        raw = self.paint(mode="Search", query="codex", panes=("codex",))
        raw = raw.replace(b"codex", b"cod\x1b[31mex\x1b[m")
        console = self.console(*(bytes([byte]) for byte in raw))
        actual = console.until_table(mode="Search", query="codex", selected=(0, "codex"), panes=("codex",), capture=False)
        self.assertEqual(actual["query"], "codex")
        self.assertEqual(actual["panes"], ("codex",))

    def test_opt_out_checks_current_delivery_state_not_historical_outer_enable_bytes(self):
        raw = b"\x1b[?1003;1006h" + self.paint(capture=False)
        console = self.console(raw)
        actual = console.until_table(mode="Select", query="f to search", selected=(0, "Claude"), panes=self.panes, capture=False)
        self.assertFalse(actual["capture"])
        self.assertIn(b"\x1b[?1003;1006h", console.output)

    def test_table_wait_keeps_transport_errors_and_five_second_deadline(self):
        console = self.console(OSError("fixture read failed"))
        with self.assertRaisesRegex(OSError, "fixture read failed"):
            self.selected(console, (0, "Claude"))
        console = self.console()
        console.queue = Mock()
        console.queue.get.side_effect = queue.Empty
        with patch.dict(Console.until_table.__globals__, time=Mock(monotonic=Mock(side_effect=[0, 0]))):
            with self.assertRaisesRegex(AssertionError, "missing current table"):
                self.selected(console, (0, "Claude"))
        console.queue.get.assert_called_once_with(timeout=5)


if __name__ == "__main__":
    unittest.main()
