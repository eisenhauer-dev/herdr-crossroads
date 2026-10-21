"""Portable marker-matching checks; native ConPTY APIs are not invoked."""
import queue
from pathlib import Path
import runpy
import unittest
from unittest.mock import Mock, patch


with patch("ctypes.WinDLL", create=True, return_value=Mock()):
    Console = runpy.run_path(str(Path(__file__).with_name("windows_support.py")))["Console"]


class ConsoleMarkerTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
