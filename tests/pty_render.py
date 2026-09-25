"""Linux PTY screen regression for narrow raw-mode popup rendering."""
import errno
import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import unicodedata
from pathlib import Path


def frame(master, marker):
    data = bytearray()
    while marker not in data:
        ready, _, _ = select.select([master], [], [], 3)
        assert ready, f"no frame ending in {marker!r}: {data[-100:]!r}"
        data.extend(os.read(master, 65536))
    while select.select([master], [], [], 0.05)[0]:
        try:
            data.extend(os.read(master, 65536))
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            break
    return data.decode("utf-8", "replace")


def screen(data, width=76, height=12, track_bg=False, track_cursor=False):
    cells = [[" "] * width for _ in range(height)]
    backgrounds = [[False] * width for _ in range(height)]
    selected_bg = False
    cursor_visible = True
    x = y = 0
    wraps = 0
    i = 0
    while i < len(data):
        c = data[i]
        if data.startswith("\x1b[", i):
            end = i + 2
            while end < len(data) and not data[end].isalpha():
                end += 1
            assert end < len(data), "incomplete ANSI escape"
            command = data[end]
            assert command in "HJmhl", command
            params = data[i + 2:end]
            if command == "H":
                if params:
                    row, col = params.split(";")
                    assert row.isdecimal() and col.isdecimal() and 1 <= int(row) <= height and 1 <= int(col) <= width, params
                    y, x = int(row) - 1, int(col) - 1
                else:
                    x = y = 0
            elif command == "J":
                assert params == "", params
                cells = [[" "] * width for _ in range(height)]
                backgrounds = [[False] * width for _ in range(height)]
            elif command == "m":
                sgr = params
                if sgr == "48;5;236":
                    selected_bg = True
                elif sgr in ("0", "49"):
                    selected_bg = False
            else:
                assert params == "?25", (params, command)
                cursor_visible = command == "h"
            i = end + 1
            continue
        if c == "\r":
            x = 0
        elif c == "\n":
            y = min(y + 1, height - 1)
        else:
            char_width = 0 if unicodedata.combining(c) else 2 if unicodedata.east_asian_width(c) in "WF" else 1
            if x >= width:
                wraps += 1
                x = 0
                y = min(y + 1, height - 1)
            if char_width:
                cells[y][x] = c
                backgrounds[y][x] = selected_bg
                x += char_width
        i += 1
    result = (["".join(line).rstrip() for line in cells], wraps)
    if track_bg:
        return (*result, backgrounds)
    return (*result, (cursor_visible, x, y)) if track_cursor else result


def assert_selected_background(raw, width, height):
    lines, wraps, backgrounds = screen(raw, width, height, track_bg=True)
    assert wraps == 0, f"wrapped {wraps} times: {lines}"
    selected = [at for at, line in enumerate(lines) if line.startswith("  > ")]
    assert len(selected) == 1, lines
    border = [x for x, char in enumerate(lines[1]) if char == "─"]
    assert border == list(range(2, width - 2)), (width, border)
    if height > 6:
        assert border == [x for x, char in enumerate(lines[height - (2 if width >= 138 else 3)]) if char == "─"], lines
    painted = [x for x, enabled in enumerate(backgrounds[selected[0]]) if enabled]
    assert painted == border, (width, selected, painted, border)
    assert all(not any(row) for at, row in enumerate(backgrounds) if at != selected[0]), backgrounds
    return lines


def main(binary):
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-screen-") as path:
        directory = Path(path)
        fixture = Path(__file__).parent / "fixtures" / "topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\ncat '{fixture}'\n")
        fake.chmod(0o700)
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 76, 0, 0))
        env = dict(os.environ, HERDR_BIN_PATH=str(fake), HERDR_PANE_ID="p-self")
        env.pop("HERDR_PLUGIN_CONFIG_DIR", None)
        env.pop("HERDR_SOCKET_PATH", None)
        child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
        os.close(slave)
        try:
            raw = frame(master, b"Filter: ")
            assert "\x1b[?25l" in raw and "\x1b[?25h" not in raw, raw[-100:]
            _, wraps, cursor = screen(raw, 76, 15, track_cursor=True)
            assert wraps == 0 and cursor[0] is False, cursor
            lines = assert_selected_background(raw, 76, 15)
            selected_raw = next(line for line in raw.splitlines() if line.startswith("  \x1b[48;5;236m> "))
            assert "Claude \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking\x1b[39m" in selected_raw, raw
            assert selected_raw.endswith("\x1b[0m"), selected_raw
            assert "codex \x1b[2m|\x1b[22m \x1b[38;2;166;227;161midle\x1b[39m" in raw, raw
            assert lines[0] == "Filter: / to search" and not any("Herdr Crossroads" in line for line in lines), lines
            assert lines[1].startswith("  ─") and all(header in lines[2] for header in ("Workspace", "Tab", "Agent/Pane Name")) and "Status" not in lines[2], lines
            assert lines[12].startswith("  ─") and not any("│" in line or "├" in line or "▏" in line for line in lines), lines
            assert sum("┄" in line for line in lines) == 1, lines
            assert lines[8].startswith("  ┄") and "codex" in lines[9], lines
            assert lines[3].startswith("  > ") and "Claude" in lines[3], lines
            assert any("Same" in line and "Claude" in line and "working" in line for line in lines), lines
            assert any("Same" in line and "Claude" not in line and "working" not in line for line in lines[3:8]), lines
            assert not any(pane_id in "\n".join(lines) for pane_id in ("p-b", "p-a", "p-other")), lines
            assert any("Filter:" in line for line in lines), lines
            assert lines[13] == "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace" and lines[14] == "Enter: focus · Esc/q: exit · ?: keys", lines
            os.write(master, b"\t")
            lines = assert_selected_background(frame(master, b"Filter: "), 76, 15)
            assert any(line.startswith("  > ") and "codex" in line for line in lines), lines
            os.write(master, b"\x1b[Z")
            lines = assert_selected_background(frame(master, b"Filter: "), 76, 15)
            assert any(line.startswith("  > ") and "Claude" in line for line in lines), lines
            for width in (150, 139, 138):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, width, 0, 0))
                os.write(master, b"r")
                lines = assert_selected_background(frame(master, b"Filter: "), width, 15)
                assert "Status" in lines[2] and lines[3].startswith("  ┄") and "Claude" in lines[4], lines
                assert lines[14] == "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace · Enter: focus · Esc/q: exit · ?: keys", lines
            for width in (137, 60, 40):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, width, 0, 0))
                os.write(master, b"r")
                lines = assert_selected_background(frame(master, b"Filter: "), width, 15)
                assert lines[3].startswith("  > "), lines
                assert lines[8].startswith("  ┄"), lines
                if width == 137:
                    assert "Status" not in lines[2] and lines[13] == "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace" and lines[14] == "Enter: focus · Esc/q: exit · ?: keys", lines
                if width == 60:
                    assert lines[13] == "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace" and lines[14] == "Enter: focus · Esc/q: exit · ?: keys", lines
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 5, 60, 0, 0))
            os.write(master, b"jjjjj")
            lines = assert_selected_background(frame(master, b"Filter: "), 60, 5)
            assert any("Same" in line and "codex" in line and "idle" in line for line in lines), lines
            assert lines[1].startswith("  ─") and lines[2].startswith("  > ") and not any("┄" in line or "▏" in line for line in lines), lines
            assert lines[3] == "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace" and lines[4] == "Enter: focus · Esc/q: exit · ?: keys", lines
            os.write(master, b"/?")
            raw = frame(master, b"Search: ?")
            lines, wraps, cursor = screen(raw, width=60, height=5, track_cursor=True)
            assert wraps == 0, lines
            assert lines[0] == "Search: ?" and "No matching panes" in "\n".join(lines) and lines[4].startswith("Enter/Esc: select"), lines
            assert cursor == (True, 9, 0), cursor
            assert not any("Herdr Crossroads" in line for line in lines), lines
            os.write(master, "界".encode())
            raw = frame(master, "Search: ?界".encode())
            lines, wraps, cursor = screen(raw, width=60, height=5, track_cursor=True)
            assert wraps == 0 and lines[0] == "Search: ?界" and cursor == (True, 11, 0), (lines, cursor)
            os.write(master, b"\x7f")
            raw = frame(master, b"Search: ?")
            _, wraps, cursor = screen(raw, width=60, height=5, track_cursor=True)
            assert wraps == 0 and cursor == (True, 9, 0), cursor
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 5, 20, 0, 0))
            os.write(master, b"a" * 40)
            raw = frame(master, b"\x1b[?25h")
            lines, wraps, cursor = screen(raw, width=20, height=5, track_cursor=True)
            assert wraps == 0 and lines[0].startswith("Search: ?") and lines[0].endswith("…") and cursor == (True, len(lines[0]), 0), (lines, cursor)
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 5, 9, 0, 0))
            os.write(master, "界".encode())
            raw = frame(master, b"\x1b[?25h")
            lines, wraps, cursor = screen(raw, width=9, height=5, track_cursor=True)
            assert wraps == 0 and lines[0].endswith("…") and cursor == (True, len(lines[0]), 0) and cursor[1] < 9, (lines, cursor)
            os.write(master, b"\x1b")
            raw = frame(master, b"\x1b[?25l")
            _, wraps, cursor = screen(raw, width=9, height=5, track_cursor=True)
            assert wraps == 0 and cursor[0] is False and "\x1b[?25l" in raw, cursor
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 5, 60, 0, 0))
            os.write(master, b"?")
            raw = frame(master, b"Esc/q/?: back")
            lines, wraps, cursor = screen(raw, width=60, height=5, track_cursor=True)
            assert wraps == 0 and lines[0].startswith("Filter:") and lines[4] == "Esc/q/?: back", lines
            assert cursor[0] is False, cursor
            assert not any("Herdr Crossroads" in line for line in lines), lines
            os.write(master, b"\x1b")
            raw = frame(master, b"Filter: ")
            assert screen(raw, width=60, height=5, track_cursor=True)[2][0] is False
            os.write(master, b"/")
            raw = frame(master, b"Backspace: erase")
            lines, wraps, cursor = screen(raw, width=60, height=5, track_cursor=True)
            assert wraps == 0 and cursor == (True, len(lines[0]) + 1, 0), (lines, cursor)
            os.write(master, b"\n")
            raw = frame(master, b"Filter: ")
            assert screen(raw, width=60, height=5, track_cursor=True)[2][0] is False
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 5, 140, 0, 0))
            os.write(master, b"r")
            lines, wraps = screen(frame(master, b"Filter: "), width=140, height=5)
            assert wraps == 0, lines
            assert lines[1].startswith("  ─") and lines[2].startswith("  > ") and not any("┄" in line or "▏" in line for line in lines), lines
            assert any("Same" in line and "codex" in line and "idle" in line for line in lines), lines
            assert lines[4] == "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace · Enter: focus · Esc/q: exit · ?: keys", lines
            os.write(master, b"q")
            raw = frame(master, b"\x1b[?25h")
            assert raw.endswith("\x1b[?25h"), raw[-100:]
            assert child.wait(timeout=3) == 0
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
            os.close(master)

        for key, expected_status in ((b"\x1b", 0), (b"\n", 1)):
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 5, 60, 0, 0))
            saved = termios.tcgetattr(slave)
            child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
            try:
                initial = frame(master, b"?: keys")
                assert "\x1b[?25l" in initial, initial[-100:]
                os.write(master, key)
                restored = frame(master, b"\x1b[?25h")
                assert "\x1b[?25h" in restored, restored[-100:]
                assert (child.wait(timeout=3) == 0) == (expected_status == 0), key
                assert termios.tcgetattr(slave) == saved, key
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
                os.close(master)
                os.close(slave)


if __name__ == "__main__":
    main(sys.argv[1])
