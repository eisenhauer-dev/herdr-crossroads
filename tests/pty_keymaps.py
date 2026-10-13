"""Isolated A10c protocol, search, error-page and long-binding PTY regressions."""
import errno
import fcntl
import json
import os
import pty
import select
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
from pathlib import Path

sys.dont_write_bytecode = True
from pty_render import expected_search_row, screen


def read_until(fd, marker, timeout=3):
    data = bytearray()
    deadline = time.monotonic() + timeout
    while marker not in data:
        remaining = deadline - time.monotonic()
        assert remaining > 0 and select.select([fd], [], [], remaining)[0], (marker, data[-300:])
        data.extend(os.read(fd, 65536))
    return bytes(data)


def read_frame(fd, expected=None):
    if expected is not None:
        # Replay draws once per key. Wait for the requested state, not an output
        # gap, and leave subsequent frames (notably timed Esc) in the PTY.
        data = bytearray()
        deadline = time.monotonic() + 3
        while True:
            remaining = deadline - time.monotonic()
            assert remaining > 0 and select.select([fd], [], [], remaining)[0], data[-300:]
            byte = os.read(fd, 1)
            assert byte, ("EOF before expected frame", data[-300:])
            data.extend(byte)
            if data.endswith((b"\x1b[?25h", b"\x1b[?25l")):
                raw = data.decode("utf-8", "replace")
                if expected(raw):
                    return raw
    data = bytearray(read_until(fd, b"\x1b[?25"))
    while select.select([fd], [], [], 0.01)[0]:
        data.extend(os.read(fd, 65536))
    return data.decode("utf-8", "replace")


def drain(fd):
    data = bytearray()
    while select.select([fd], [], [], 0.2)[0]:
        try:
            data.extend(os.read(fd, 65536))
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            break
    return bytes(data)


def expected_source_row(directory, filename, width):
    available = max(0, width - 1)
    if available < len(filename) + 4:
        return filename if len(filename) <= available else filename[:max(0, available - 1)] + ("…" if available else "")
    path = str(directory / filename)
    budget = available - len(filename) - 2
    if len(path) > budget:
        path = path[:budget - 1] + "…"
    return filename + "  " + path


def assert_source_rows(lines, directory, width, height):
    if height <= 2:
        assert not any("/tmp/opencode" in line or ".jsonc" in line for line in lines), lines
        return
    source_rows = {expected_source_row(directory, filename, width) for filename in ("keymaps.jsonc", "status-colors.jsonc", "theme.jsonc")}
    for at, line in enumerate(lines[:-2]):
        if any(token in line for token in ("/tmp/", "opencode", str(directory.name), ".jsonc")):
            assert line in source_rows, f"source repeated within diagnostic: {lines}"
        if line.startswith("ERROR:") and "Popup too small" not in line and at + 1 < height - 2:
            assert lines[at + 1] in source_rows, f"missing source under heading: {lines}"


class Popup:
    def __init__(self, binary, directory, config=None, kitty=False, width=80, height=15, focus_socket=None, probe_input=b"", initial_frame=None):
        self.width, self.height = width, height
        keymap = directory / "keymaps.jsonc"
        if config is not None:
            keymap.write_text(config if isinstance(config, str) else json.dumps(config))
        elif keymap.exists():
            keymap.unlink()
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
        env = dict(os.environ, HERDR_BIN_PATH=str(directory / "herdr"), HERDR_PLUGIN_CONFIG_DIR=str(directory), HERDR_CONFIG_PATH=str(directory / "missing"), HERDR_PANE_ID="p-self")
        env.pop("HERDR_SOCKET_PATH", None)
        if focus_socket:
            env["HERDR_SOCKET_PATH"] = str(focus_socket)
        self.child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
        os.close(slave)
        try:
            query = read_until(self.master, b"\x1b[?u")
            assert b"\x1b[>1u" not in query
            if probe_input:
                os.write(self.master, probe_input)
            if kitty:
                os.write(self.master, b"\x1b[?0u")
            self.initial = read_frame(self.master, expected=initial_frame)
            assert ("\x1b[>1u" in self.initial) == kitty, self.initial
            self.kitty = kitty
        except BaseException:
            self.abort()
            raise

    def send(self, data):
        os.write(self.master, data)
        raw = read_frame(self.master)
        lines, wraps = screen(raw, self.width, self.height)
        assert wraps == 0, lines
        return lines

    def finish(self, key, success=True):
        os.write(self.master, key)
        assert (self.child.wait(timeout=3) == 0) == success
        tail = drain(self.master)
        assert tail.count(b"\x1b[<u") == int(self.kitty), tail
        assert b"\x1b[?25h" in tail, tail
        os.close(self.master)

    def abort(self):
        if self.child.poll() is None:
            self.child.kill()
            self.child.wait()
        try:
            os.close(self.master)
        except OSError:
            pass


def frame_reader_cases():
    navigation = b"\x1b[H\x1b[J  Select: pane 3\x1b[?25l"
    search = b"\x1b[H\x1b[J  Search: codex\r\nBackspace: erase\x1b[?25h"
    escaped = b"\x1b[H\x1b[J  Search: codex\r\nNavigation\x1b[?25l"
    for coalesced in [False, True]:
        reader, writer = socket.socketpair()
        replay = threading.Event()
        def emit():
            if coalesced:
                writer.sendall(navigation + search + escaped)
            else:
                writer.sendall(navigation)
                assert replay.wait(3), "reader did not observe the intermediate frame"
                writer.sendall(search + escaped)
        worker = threading.Thread(target=emit)
        worker.start()
        try:
            def searching(raw):
                if raw.endswith(navigation.decode()):
                    replay.set()
                return raw.endswith(search.decode())
            raw = read_frame(reader.fileno(), expected=searching)
            assert raw == (navigation + search).decode(), raw
            raw = read_frame(reader.fileno(), expected=lambda raw: raw.endswith(escaped.decode()))
            assert raw == escaped.decode(), raw
        finally:
            replay.set()
            worker.join(timeout=3)
            reader.close()
            writer.close()
        assert not worker.is_alive()


def main(binary):
    frame_reader_cases()
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-keys-") as temporary:
        directory = Path(temporary)
        fixture = Path(__file__).parent.resolve() / "fixtures/topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\nif [ \"$*\" = 'pane list' ]; then exit 9; fi\nif [ \"$*\" = 'api snapshot' ]; then cat '{fixture}'; else echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; fi\n")
        fake.chmod(0o700)
        input_protocol_cases(binary, directory)
        if "--input-protocol" in sys.argv[2:]:
            print("A10c package 1 PTY: repeated Esc, delayed Esc/q, Alt+q, late Kitty reply, buffered probe input passed")
            return
        binding_policy_cases(binary, directory)
        themes_help_cases(binary, directory)
        error_block_cases(binary, directory)
        long_source_block_cases(binary, directory)
        popup = None
        try:
            for kitty in [False, True]:
                popup = Popup(binary, directory, kitty=kitty)
                for help_search in [False, True]:
                    if help_search:
                        popup.send(b"?")
                    popup.send(b"/x")
                    start = time.monotonic()
                    lines = popup.send(b"\x1bx")
                    assert time.monotonic() - start < 0.095, "complete Alt input must not wait for Esc timeout"
                    assert lines[0] == expected_search_row(80, "x", "Help" if help_search else "Search") and "Backspace: erase" in lines[-1], lines
                    keys = b"\n\x08\t" if not kitty else b"\x1b[106;5u\x1b[104;5u\x1b[105;5u\x1b[109;5u"
                    os.write(popup.master, keys)
                    raw = read_frame(popup.master) + drain(popup.master).decode("utf-8", "replace")
                    lines, wraps = screen(raw, 80, 15)
                    assert wraps == 0 and lines[0] == expected_search_row(80, "x", "Help" if help_search else "Search") and "Backspace: erase" in lines[-1], lines
                    lines = popup.send(b"\x7f")
                    assert lines[0] == expected_search_row(80, "", "Help" if help_search else "Search"), lines
                    # Raw Esc has a fixed 100ms ambiguity deadline in either mode.
                    start = time.monotonic()
                    os.write(popup.master, b"\x1b")
                    assert not select.select([popup.master], [], [], 0.075)[0], "Esc resolved early"
                    read_frame(popup.master)
                    assert 0.09 <= time.monotonic() - start < 0.22
                    if help_search:
                        popup.send(b"q")
                popup.finish(b"q")

            config = {"select": {"close": ["Ctrl+i"], "next_row": ["Ctrl+m"], "open_help": ["Alt+x"]}}
            popup = Popup(binary, directory, config, width=35, height=7)
            assert "ERROR: Popup too small" in popup.initial and "minimum content" not in popup.initial
            combined = popup.initial
            for _ in range(30):
                combined += "\n".join(popup.send(b"\x1b[B"))
            assert "Ctrl/Alt bindings are not allowed" in " ".join(combined.split()) and "Ctrl+i" in combined, combined
            popup.finish(b"\r")

            config = {"select": {f"unknown-{i}": ["F1"] for i in range(25)}}
            config["select"].update({"focus_pane": ["j", "Down"], "open_search": ["k", "Up"]})
            popup = Popup(binary, directory, config, width=42, height=8, kitty=True)
            assert "ERROR: Popup too small" in popup.initial and "minimum content" not in popup.initial
            first, _ = screen(popup.initial, 42, 8)
            assert_source_rows(first, directory, 42, 8)
            down = popup.send(b"j")
            assert down != first, (first, down)
            assert popup.send(b"k") == first
            assert popup.send(b"\x1b[B") == down
            for modified in (b"\x1b[106;5u", b"\x1b[106;3u", b"\x1b[107;5u", b"\x1b[107;3u"):
                assert popup.send(modified) == down
            assert popup.send(b"\x1b[A") == first
            combined = ""
            for _ in range(150):
                combined += "\n".join(popup.send(b"\x1b[B"))
            assert "unknown-24" in combined, combined
            popup.finish(b"q")

            # Oversized lists are rejected before dedup/display, rather than paged.
            long_keys = [f"Ctrl+{c}" for c in "abcdefghjkl nopqrstuvwxyz".replace(" ", "")] + [f"Alt+{c}" for c in "abcdefghijklmnopqrstuvwxyz"]
            popup = Popup(binary, directory, {"select": {"next_row": long_keys}}, kitty=True)
            assert popup.initial.count("Maximum 2 keybinds per action - select mode") == 1 and "Context |" not in popup.initial and "next_row" in popup.initial, popup.initial
            assert "ERROR: Maximum 2 keybinds per action - select mode" in popup.initial, popup.initial
            assert_source_rows(screen(popup.initial, 80, 15)[0], directory, 80, 15)
            assert "Agent/Pane Name" not in popup.initial, popup.initial
            popup.finish(b"q")

            for mixed in (False, True):
                config = {"select": {"next_row": ["J"] * 3, "previous_row": ["K"] * 3}, "help": {"next_category": ["A", "B", "C"]}}
                if mixed:
                    config["select"].update({"next_tab": ["F1", "q"], "broken": None})
                popup = Popup(binary, directory, config, kitty=True)
                assert popup.initial.count("Maximum 2 keybinds per action - ") == 2, popup.initial
                initial_lines, _ = screen(popup.initial, 80, 15)
                assert "ERROR: Maximum 2 keybinds per action - select mode" in initial_lines and "ERROR: Maximum 2 keybinds per action - help mode" in initial_lines, initial_lines
                assert "Context |" not in popup.initial, popup.initial
                for row in ("next_row", "previous_row", "next_category"):
                    assert row in popup.initial, popup.initial
                assert "Other diagnostics" not in popup.initial, popup.initial
                assert_source_rows(initial_lines, directory, 80, 15)
                combined = popup.initial
                for _ in range(40):
                    combined += "\n".join(popup.send(b"\x1b[B"))
                if mixed:
                    # Diagnostic rows may wrap; preserve words and owner order.
                    normalized = " ".join(combined.split())
                    for diagnostic in ("ERROR: Unknown keymap keys - select mode", "unknown key", "broken:", "next_tab, close"):
                        assert diagnostic in normalized, combined
                assert_source_rows(popup.send(b"k"), directory, 80, 15)
                popup.finish(b"\r")

            config = {"select": {"next_row": ["J"] * 3, "previous_row": ["K"] * 3}}
            config["select"].update({f"unknown-{i}": ["F1"] for i in range(10)})
            config["select"].update({"focus_pane": ["j", "Down"], "open_search": ["k", "Up"]})
            for width, height in ((35, 7), (10, 3), (40, 2), (40, 1)):
                popup = Popup(binary, directory, config, width=width, height=height, kitty=True)
                assert "ERROR" in popup.initial, popup.initial
                assert "\x1b[1m\x1b[38;2;243;139;168mERROR" in popup.initial, popup.initial
                assert "\x1b[22m\x1b[38;2;205;214;244m" in popup.initial, popup.initial
                assert "minimum content" not in popup.initial, popup.initial
                if height <= 2:
                    assert f"need 80x15; actual {width}x{height}." in popup.initial, popup.initial
                elif height > 3:
                    assert "ERROR: Popup too small" in popup.initial, popup.initial
                else:
                    assert "need" in "".join(popup.send(b"j")), popup.initial
                    popup.send(b"k")
                assert_source_rows(screen(popup.initial, width, height)[0], directory, width, height)
                if height <= 3:
                    assert "Context" not in popup.initial, popup.initial
                first, _ = screen(popup.initial, width, height)
                down = popup.send(b"j")
                assert down != first, (width, height, first, down)
                assert popup.send(b"k") == first
                assert popup.send(b"\x1b[B") == down
                assert popup.send(b"\x1b[A") == first
                popup.finish(b"q")

            # Focus failure unwinds our push as well as restoring raw terminal state.
            address = directory / "focus.sock"
            listener = socket.socket(socket.AF_UNIX)
            listener.bind(str(address))
            listener.listen()
            request = []
            def reply():
                connection, _ = listener.accept()
                with connection:
                    request.append(json.loads(connection.makefile("rb").readline()))
                    connection.sendall(b'{"id":"wrong","result":{}}\n')
            worker = threading.Thread(target=reply)
            worker.start()
            popup = Popup(binary, directory, {"select": {"next_row": ["J"], "focus_pane": ["F"]}}, kitty=True, focus_socket=address)
            initial, _ = screen(popup.initial, 80, 15)
            for modified in (b"\x1b[106;5u", b"\x1b[102;5u", b"\x1b[109;5u", b"\x1b[1;5B"):
                assert popup.send(modified) == initial, "modified input must not move or focus"
                assert not request and popup.child.poll() is None
            lines = popup.send(b"J")
            assert any(line.startswith("  >") and "Claude" not in line for line in lines), lines
            popup.finish(b"F", success=False)
            worker.join(timeout=3)
            listener.close()
            assert request and request[0]["method"] == "pane.focus" and request[0]["params"]["pane_id"] == "p-a", request
        finally:
            if popup:
                popup.abort()
    print("A10c PTY: classic/Kitty, searches, Esc/Alt timing, oversized-list errors, cleanup passed")


def binding_policy_cases(binary, directory):
    for kitty in (False, True):
        for context, action in (("select", "next_row"), ("help", "next_category"), ("themes", "next_theme")):
            for keys in (["Ctrl+j", "Alt+x"], ["Ctrl+i", "Ctrl+m"], ["Ctrl+Up", "Alt+Down"], ["Ctrl+Left", "Alt+Right"]):
                popup = Popup(binary, directory, {context: {action: keys}}, kitty=kitty, width=160, height=24)
                try:
                    lines, wraps = screen(popup.initial, 160, 24)
                    assert wraps == 0 and "Agent/Pane Name" not in popup.initial, lines
                    heading = f"ERROR: Unsupported keymap keys - {context} mode"
                    at = lines.index(heading)
                    assert lines[at + 1] == expected_source_row(directory, "keymaps.jsonc", 160), lines
                    for key in keys:
                        assert f'{action}: "{key}": Ctrl/Alt bindings are not allowed' in lines, lines
                    assert not any(f"{context}." in line for line in lines), lines
                    first = lines
                    assert popup.send(b"\x1b[109;5u") == first
                    assert popup.child.poll() is None
                    popup.finish(b"q")
                finally:
                    popup.abort()
        for keys, label in ((["/"], "/"), (["/", "s"], "'/'/s"), (["s", "/"], "s/'/'"), (["|"], "'|'")):
            popup = Popup(binary, directory, {"select": {"open_search": keys}, "help": {"open_search": keys}}, kitty=kitty, width=160, height=24)
            try:
                lines, _ = screen(popup.initial, 160, 24)
                assert lines[0] == expected_search_row(160, f"{label} to search", "Select"), lines
                lines = popup.send(b"?")
                assert lines[0] == expected_search_row(160, f"{label} to search", "Help"), lines
                assert lines[1] == "  Keys — Select", lines
                assert any(line.split() == [label, "search"] for line in lines), lines
                popup.send(keys[0].encode())
                lines = popup.send(label.encode())
                assert any("search" in line and label in line for line in lines[3:-2]), lines
                popup.send(b"\r")
                popup.send(b"q")
                popup.finish(b"q")
            finally:
                popup.abort()
        # Raw HT and CR retain their byte identities even after chord rejection.
        popup = Popup(binary, directory, kitty=kitty)
        try:
            lines = popup.send(b"\t")
            assert any(line.startswith("  >") and "codex" in line for line in lines), lines
            popup.send(b"/x")
            lines = popup.send(b"\r")
            assert lines[0] == expected_search_row(80, "x", "Select") and "Backspace: erase" not in lines[-1], lines
            popup.finish(b"q")
        finally:
            popup.abort()


def themes_help_cases(binary, directory):
    theme = directory / "theme.jsonc"
    saved = '{"theme":"dracula"}'
    theme.write_text(saved)
    address = directory / "themes-help-focus.sock"
    try:
        with socket.socket(socket.AF_UNIX) as listener:
            listener.bind(str(address))
            listener.listen()
            listener.setblocking(False)
            for width, height in ((80, 15), (142, 30)):
                popup = Popup(binary, directory, width=width, height=height, focus_socket=address)
                try:
                    popup.send(b"/Same")
                    popup.send(b"\r")
                    select_before = popup.send(b"j")
                    assert select_before[0] == expected_search_row(width, "Same", "Select"), select_before
                    assert any(line.startswith("  >") and line.endswith("Same") for line in select_before), select_before
                    popup.send(b"t")
                    for preview in (False, True):
                        # Change the pending cursor without saving the prior choice.
                        before = popup.send(b"j")
                        if preview:
                            before = popup.send(b" ")
                            assert before[1] == expected_search_row(width, "Same", "Themes"), before
                            assert any("Agent/Pane Name" in line for line in before), before
                        else:
                            assert before[0] == "  Themes · selected: dracula · preview: nord", before
                            assert any(line.startswith("  > nord") for line in before), before
                            assert not any("Search:" in line for line in before), before
                        help_lines = popup.send(b"?")
                        assert help_lines[0] == expected_search_row(width, "/ to search", "Help"), help_lines
                        assert help_lines[1] == "  Keys — Themes", help_lines
                        assert any("save theme" in line and "Enter" in line for line in help_lines), help_lines
                        assert not any("focus selected pane" in line or "next workspace" in line for line in help_lines), help_lines
                        popup.send(b"/")
                        searching = popup.send(b"?")
                        assert searching[0] == expected_search_row(width, "?", "Help"), searching
                        assert searching[1] == "  Keys — Themes", searching
                        assert any("show keys" in line for line in searching), searching
                        assert searching[-1] == "  Enter/Esc: return to keys · Backspace: erase", searching
                        navigation = popup.send(b"\x1b")
                        assert navigation[0] == expected_search_row(width, "?", "Help") and navigation[1] == "  Keys — Themes", navigation
                        assert "close keys" in navigation[-1], navigation
                        assert popup.send(b"q") == before, "nested help changed theme view, cursor or pane preview"
                        assert theme.read_text() == saved, "help saved or canceled a pending theme"
                    assert popup.send(b"\x1b") == select_before, "help changed remembered pane query, selection or viewport"
                    popup.finish(b"q")
                finally:
                    popup.abort()
            try:
                connection, _ = listener.accept()
            except BlockingIOError:
                pass
            else:
                connection.close()
                raise AssertionError("Themes help must never focus a pane")
    finally:
        theme.unlink()


def error_block_cases(binary, directory):
    config = {"select": {"next_row": ["J"] * 3, "next_tab": ["q"], "broken": None}}
    (directory / "status-colors.jsonc").write_text('{"working":false}')
    (directory / "theme.jsonc").write_text('{"theme":"bogus"}')
    popup = None
    try:
        popup = Popup(binary, directory, config, kitty=True, width=240, height=36)

        def check(raw, width, height):
            lines, wraps = screen(raw, width, height)
            assert wraps == 0, (width, height, lines)
            assert_source_rows(lines, directory, width, height)
            assert "Other diagnostics" not in raw and "ERROR: Configuration" not in raw and "ERROR: Keymap configuration" not in raw, raw
            assert "minimum content" not in raw, raw
            if height >= 3:
                assert lines[-2] == "─" * width, (width, height, lines)
                assert lines[-1].startswith("↑/k:"), lines
                if width >= 80:
                    assert lines[-1] == "↑/k: scroll up · ↓/j: scroll down · Enter/Esc/q: close", lines
                for row in raw.split("\r\n"):
                    if "ERROR:" in row:
                        assert "\x1b[1m\x1b[38;2;243;139;168mERROR:" in row, row
                        assert "\x1b[22m\x1b[38;2;205;214;244m" in row, row
            else:
                assert all(line.startswith("ERROR:") for line in lines), lines
                assert not any("─" in line for line in lines), lines
                assert not any("scroll up" in line for line in lines), lines
            return lines

        initial = check(popup.initial, 240, 36)
        for heading, filename in (("Maximum 2 keybinds per action - select mode", "keymaps.jsonc"), ("Unknown keymap actions - select mode", "keymaps.jsonc"), ("Keymap structure - select mode", "keymaps.jsonc"), ("Keymap collisions - select mode", "keymaps.jsonc"), ("Status colors invalid values", "status-colors.jsonc"), ("Theme invalid values", "theme.jsonc")):
            at = initial.index("ERROR: " + heading)
            assert initial[at + 1] == f"{filename}  {directory / filename}", initial
            source_raw = next(row for row in popup.initial.split("\r\n") if f"{filename}  " in row)
            assert f"{filename}  \x1b[38;2;108;112;134m\x1b[2m" in source_raw, source_raw
            assert "\x1b[22m\x1b[38;2;205;214;244m" in source_raw, source_raw
            end = next(row for row in range(at + 1, len(initial)) if initial[row].startswith("─"))
            assert sum(filename in line for line in initial[at + 1:end]) == 1, initial
        for heading in ("Maximum 2 keybinds per action - select mode", "Unknown keymap actions - select mode", "Keymap structure - select mode", "Keymap collisions - select mode", "Status colors invalid values", "Theme invalid values"):
            assert "ERROR: " + heading in initial, initial
        assert initial[-3] == "", "short content must leave padding above fixed footer"
        for width, height in ((67, 12), (79, 15), (80, 14), (80, 15), (40, 3), (40, 2), (40, 1), (162, 36), (80, 15)):
            fcntl.ioctl(popup.master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
            popup.width, popup.height = width, height
            check(read_frame(popup.master), width, height)
            for _ in range(100):
                os.write(popup.master, b"k")
                first = check(read_frame(popup.master), width, height)
            small = width < 80 or height < 15
            if height > 2:
                assert first[0].startswith("ERROR: Popup too small") == small, (width, height, first)
            collected = "".join(first[:-2] if height >= 3 else first)
            for _ in range(50):
                os.write(popup.master, b"j")
                lines = check(read_frame(popup.master), width, height)
                collected += "".join(lines[:-2] if height >= 3 else lines)
            assert "unknownthemebogus" in collected.replace(" ", ""), (width, height, collected)
            normalized = collected.replace(" ", "").replace("ERROR:", "")
            assert ("need80x15" in normalized) == small, (width, height, collected)
            if small:
                assert f"actual{width}x{height}" in normalized and "Enlargepopupandreopen." in normalized, collected
            for prefix in ("select.", "select:", "select |"):
                assert prefix not in collected, collected
        popup.finish(b"q")
    finally:
        if popup:
            popup.abort()
        (directory / "status-colors.jsonc").unlink()
        (directory / "theme.jsonc").unlink()


def long_source_block_cases(binary, directory):
    for kind, extension in (("fields", "jsonc"), ("syntax", "jsonc"), ("syntax", "json"), ("io", "json")):
        with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-source-") as temporary:
            config_dir = Path(temporary) / ("long-source-component-" * 7)
            config_dir.mkdir()
            (config_dir / "herdr").write_text((directory / "herdr").read_text())
            (config_dir / "herdr").chmod(0o700)
            for name, content in (("keymaps", '{"select":{"next_row":[]}}'), ("status-colors", '{"working":false}'), ("theme", '{"theme":"bogus"}')):
                target = config_dir / f"{name}.{extension}"
                if kind == "io":
                    target.mkdir()
                else:
                    target.write_text("{" if kind == "syntax" else content)
            config = None if extension == "json" else "{" if kind == "syntax" else {"select": {"next_row": []}}
            for width, height in ((240, 36), (67, 12), (40, 3), (40, 1), (80, 15)):
                popup = Popup(binary, config_dir, config, kitty=True, width=width, height=height)
                try:
                    lines, wraps = screen(popup.initial, width, height)
                    assert wraps == 0, lines
                    rows = []
                    for _ in range(1000):
                        rows.append(lines[0])
                        down = popup.send(b"j")
                        if height >= 3:
                            assert lines[-2] == down[-2] == "─" * width, lines
                            assert lines[-1] == down[-1], (lines, down)
                        else:
                            assert all(line.startswith("ERROR:") for line in down), down
                            assert not any("─" in line for line in down), down
                        # Tiny IO pages contain two size details followed by
                        # three identical one-row IO details, without spacers.
                        # Identical adjacent frames cannot signal scroll end.
                        tiny_io = height <= 2 and kind == "io"
                        at_end = len(rows) == 2 + 3 - (height - 1) if tiny_io else down == lines
                        if at_end:
                            assert down == lines, (lines, down)
                            rows.extend(lines[1:height - 2] if height >= 3 else lines[1:])
                            break
                        lines = down
                    else:
                        raise AssertionError("source and diagnostics did not reach scroll end")
                    text = "".join(rows).replace("ERROR: ", "")
                    if height >= 3:
                        for name in ("keymaps", "status-colors", "theme"):
                            filename = f"{name}.{extension}"
                            source = expected_source_row(config_dir, filename, width)
                            assert rows.count(source) == 1, (kind, width, height, rows)
                            source_at = rows.index(source)
                            assert rows[source_at - 1].startswith("ERROR:"), rows
                            assert not rows[source_at + 1].startswith("/tmp/"), rows
                            if width < 240:
                                assert source.endswith("…"), source
                        for at, line in enumerate(rows):
                            assert not line.startswith("/tmp/"), "no wrapped source continuations"
                    else:
                        assert str(config_dir) not in text and ".json" not in text, rows
                    normalized = text.replace(" ", "")
                    if kind == "fields":
                        for diagnostic in ("next_row:expectednonemptykeylist", "workingmustbe#RRGGBB", "unknownthemebogus"):
                            assert diagnostic in normalized, rows
                    elif kind == "syntax":
                        assert normalized.count("Unterminatedobject") == 3, rows
                    else:
                        assert normalized.count("directory") == 3, rows
                    popup.finish(b"q")
                finally:
                    popup.abort()


def input_protocol_cases(binary, directory):
    address = directory / "input-focus.sock"
    with socket.socket(socket.AF_UNIX) as listener:
        listener.bind(str(address))
        listener.listen()
        listener.setblocking(False)
        popup = None
        try:
            for kitty in [False, True]:
                popup = Popup(binary, directory, kitty=kitty, focus_socket=address)
                initial, _ = screen(popup.initial, 80, 15)
                assert initial[0] == expected_search_row(80, "/ to search", "Select"), initial
                # Recognized modifier sequences must never focus, navigate, close,
                # or leak their bytes into Select/Help search or theme controls.
                modified = (b"\n", b"\x08", b"\x1bf", b"\x1b[105;5u", b"\x1b[109;5u", b"\x1b[106;3u", b"\x1b[1;5A", b"\x1b[1;3B", b"\x1b[57351;5u")
                for open_key, close_key in ((None, None), (b"?", b"q"), (b"t", b"\x1b[27u"), (b"/x", b"\r"), (b"?/x", b"\rq")):
                    lines = popup.send(open_key) if open_key else initial
                    for sequence in modified:
                        assert popup.send(sequence) == lines, (sequence, lines)
                        assert popup.child.poll() is None
                    if close_key:
                        popup.send(close_key)
                    if open_key in (b"/x", b"?/x"):
                        popup.send(b"r")
                popup.finish(b"q")

                # F1/F4: the first Esc returns from search; the second starts the arrow.
                popup = Popup(binary, directory, kitty=kitty, focus_socket=address)
                popup.send(b"j/Same")
                start = time.monotonic()
                lines = popup.send(b"\x1b\x1b[A")
                assert time.monotonic() - start < 0.095, "a complete double-Esc arrow must not wait for timeout"
                assert lines[0] == expected_search_row(80, "Same", "Select") and "Backspace: erase" not in lines[-1], lines
                assert any(line.startswith("  >") and "Claude" in line for line in lines), lines
                assert popup.child.poll() is None
                popup.send(b"/")
                # Both Esc keys must run without closing stdin or the PTY master.
                popup.finish(b"\x1b\x1b")

                # F3: rapid Esc+q is Alt+q, not return/close; q cannot become search text.
                popup = Popup(binary, directory, kitty=kitty, focus_socket=address)
                popup.send(b"/codex")
                lines = popup.send(b"\x1bq")
                assert lines[0] == expected_search_row(80, "codex", "Search") and "Backspace: erase" in lines[-1], lines
                assert popup.child.poll() is None, "Alt+q must not close search"
                # Now exercise actual standalone Esc, then q, with stdin still open.
                start = time.monotonic()
                os.write(popup.master, b"\x1b")
                assert not select.select([popup.master], [], [], 0.075)[0], "Esc resolved before its deadline"
                raw = read_frame(popup.master)
                lines, wraps = screen(raw, 80, 15)
                assert wraps == 0 and lines[0] == expected_search_row(80, "codex", "Select"), lines
                assert "Backspace: erase" not in lines[-1], lines
                assert 0.09 <= time.monotonic() - start < 0.22
                assert popup.child.poll() is None, "Esc must return to navigation, not close"
                popup.finish(b"q")

            # F4: a reply after the opening probe must not enable Kitty or remove configuration errors.
            popup = Popup(binary, directory, {"select": {"close": ["Ctrl+i"]}}, focus_socket=address)
            assert "Unsupported keymap keys - select mode" in popup.initial and "Ctrl+i" in popup.initial, popup.initial
            os.write(popup.master, b"\x1b[?1u")
            assert not select.select([popup.master], [], [], 0.15)[0], "late Kitty response must be consumed without output or push"
            os.write(popup.master, b"\x1b[B")
            raw = read_frame(popup.master)
            assert "\x1b[>1u" not in raw and "\x1b[<u" not in raw, "late reply must not acquire protocol ownership"
            lines, wraps = screen(raw, 80, 15)
            assert wraps == 0 and any("Unsupported keymap keys - select mode" in line for line in lines) and any("Ctrl+i" in line for line in lines), lines
            popup.finish(b"q")

            def searching_codex(raw):
                lines, _ = screen(raw, 80, 15)
                return lines[0] == expected_search_row(80, "codex", "Search") and "Backspace: erase" in lines[-1]
            def navigation_codex(raw):
                lines, _ = screen(raw, 80, 15)
                return lines[0] == expected_search_row(80, "codex", "Select") and "Backspace: erase" not in lines[-1]
            def pane_three(raw):
                lines, _ = screen(raw, 80, 15)
                return any(line.startswith("  >") and "pane 3" in line for line in lines)
            for kitty in [False, True]:
                popup = Popup(binary, directory, kitty=kitty, focus_socket=address, probe_input=b"j\x1b[B", initial_frame=pane_three)
                initial, wraps = screen(popup.initial, 80, 15)
                assert wraps == 0 and any(line.startswith("  >") and "pane 3" in line for line in initial), initial
                assert "Backspace: erase" not in initial[-1], initial
                popup.finish(b"q")

                # F4: preserve input preceding the probe response, including a pending Esc.
                popup = Popup(binary, directory, kitty=kitty, focus_socket=address, probe_input=b"j\x1b[B/codex\x1b", initial_frame=searching_codex)
                initial, wraps = screen(popup.initial, 80, 15)
                assert wraps == 0 and initial[0] == expected_search_row(80, "codex", "Search") and "Backspace: erase" in initial[-1], initial
                raw = read_frame(popup.master, expected=navigation_codex)
                lines, wraps = screen(raw, 80, 15)
                assert wraps == 0 and lines[0] == expected_search_row(80, "codex", "Select") and "Backspace: erase" not in lines[-1], lines
                assert popup.child.poll() is None
                popup.finish(b"q")

            try:
                connection, _ = listener.accept()
            except BlockingIOError:
                pass
            else:
                connection.close()
                raise AssertionError("input/protocol cases must never request pane focus")
        finally:
            if popup:
                popup.abort()


if __name__ == "__main__":
    main(sys.argv[1])
