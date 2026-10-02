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


def screen(data, width=76, height=12, track_bg=False, track_cursor=False, track_panel_bg=False):
    cells = [[" "] * width for _ in range(height)]
    backgrounds = [[False] * width for _ in range(height)]
    panel_backgrounds = [[False] * width for _ in range(height)]
    selected_bg = False
    panel_bg = False
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
            assert command in "HJKmhlu", command
            params = data[i + 2:end]
            if command == "u":
                assert params in ("?", ">1", "<"), params
            elif command == "H":
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
                panel_backgrounds = [[False] * width for _ in range(height)]
                # Timed hint pages may precede a resize frame at the previous dimensions.
                # Check wraps in the final full repaint, just like the cells below.
                wraps = 0
            elif command == "K":
                assert params == "", params
                # A full-width write leaves wrap pending with the cursor on the
                # final cell, not beyond it. EL still erases that occupied cell.
                for at in range(min(x, width - 1), width):
                    cells[y][at] = " "
                    backgrounds[y][at] = False
                    panel_backgrounds[y][at] = panel_bg
            elif command == "m":
                sgr = params
                if sgr == "48;5;236":
                    selected_bg = True
                elif sgr in ("0", "49", "48;2;24;24;37"):
                    selected_bg = False
                if sgr in ("0", "49", "48;5;236"):
                    panel_bg = False
                elif sgr == "48;2;24;24;37":
                    panel_bg = True
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
                panel_backgrounds[y][x] = panel_bg
                x += char_width
        i += 1
    result = (["".join(line).rstrip() for line in cells], wraps)
    if track_bg:
        return (*result, backgrounds)
    if track_panel_bg:
        return (*result, panel_backgrounds)
    return (*result, (cursor_visible, x, y)) if track_cursor else result


def assert_selected_background(raw, width, height):
    lines, wraps, backgrounds = screen(raw, width, height, track_bg=True)
    assert wraps == 0, f"wrapped {wraps} times: {lines}"
    selected = [at for at, line in enumerate(lines) if line.startswith("  > ")]
    assert len(selected) == 1, lines
    border = [x for x, char in enumerate(lines[1]) if char == "─"]
    assert border == list(range(2, width - 2)), (width, border)
    if height > 6:
        assert border == [x for x, char in enumerate(lines[height - len(default_footer_lines(width)) - 1]) if char == "─"], lines
    painted = [x for x, enabled in enumerate(backgrounds[selected[0]]) if enabled]
    assert painted == border, (width, selected, painted, border)
    assert all(not any(row) for at, row in enumerate(backgrounds) if at != selected[0]), backgrounds
    return lines


def assert_panel_background(raw, width, height):
    lines, wraps, backgrounds = screen(raw, width, height, track_panel_bg=True)
    assert wraps == 0, (width, height, wraps, lines)
    assert all(all(row) for row in backgrounds), (width, height, [at for at, row in enumerate(backgrounds) if not all(row)], lines)
    return lines


def assert_help_frame(raw, width=80, height=15, searching=False):
    lines, wraps, cursor = screen(raw, width, height, track_cursor=True)
    assert wraps == 0 and cursor[0] == searching, (lines, wraps, cursor)
    border = "  " + "─" * (width - 4)
    footer = ["  Enter/Esc: return to keys · Backspace: erase"] if searching else default_footer_lines(width, "help")
    assert lines[1] == border and lines[-len(footer) - 1] == border, lines
    assert lines[0].startswith("  Search: "), lines
    assert not any("Keys ·" in line or "Meaning" in line or "Agent/Pane Name" in line for line in lines), lines
    if searching:
        assert lines[-1] == "  Enter/Esc: return to keys · Backspace: erase", lines
        assert "close keys" not in lines[-1], lines
    else:
        assert lines[-len(footer):] == footer, lines
    return lines


NOTICE_END = b"  Press Enter/Esc/q to close."


def default_footer_lines(width, mode="select", table=False):
    # Independent expected effective default identities, all visible without paging.
    text = {
        "select": "j/↓: ↓pane · k/↑: ↑pane · h/←: ↑tab · l/→: ↓tab · Tab: ↓ws · Shift+Tab: ↑ws · Enter: focus · Esc: exit · ?: keys",
        "help": "j/↓: ↓move · k/↑: ↑move · Esc: close keys",
        "themes": f"j/↓: ↓theme · k/↑: ↑theme · Space: {'list' if table else 'table'} · Enter: save · Esc: cancel",
    }[mode]
    wrapped = [""]
    for unit in text.split(" · "):
        candidate = wrapped[-1] + (" · " if wrapped[-1] else "") + unit
        if sum(1 if c.isascii() else 2 for c in candidate) > width - 4:
            wrapped.append(unit)
        else:
            wrapped[-1] = candidate
    budget = max(len(wrapped), 2 if mode == "themes" else 1)
    assert budget <= 3, (width, wrapped)
    return ["  " + line for line in wrapped] + [""] * (budget - len(wrapped))


def assert_footer(lines, width, mode="select", table=False):
    expected = default_footer_lines(width, mode, table)
    assert lines[-len(expected):] == expected, (lines, expected)


def notice_lines(width, height):
    if height <= 2:
        return [f"ERROR: {width}x{height}; need 80x15.", "Enlarge popup and reopen."][:height]
    return [
        "ERROR: Crossroads needs minimum content size 80x15.",
        f"  Your content size is: {width}x{height}.",
        "  Set larger size in herdr plugin manifest and reopen.",
        "  Press Enter/Esc/q to close.",
    ]


def assert_notice_style(raw):
    header = next(line for line in raw.split("\r\n") if "ERROR:" in line)
    assert "\x1b[1m\x1b[38;2;243;139;168mERROR:" in header, header
    assert "\x1b[22m\x1b[38;2;205;214;244m\x1b[48;2;24;24;37m\x1b[K" in header, header
    # A one-row frame's final terminal reset follows its background-painted row.
    assert "\x1b[0m" not in header.split("\x1b[K")[0], header


def main(binary):
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-screen-") as path:
        directory = Path(path)
        fixture = Path(__file__).parent / "fixtures" / "topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\ncat '{fixture}'\n")
        fake.chmod(0o700)
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 80, 0, 0))
        env = dict(os.environ, HERDR_BIN_PATH=str(fake), HERDR_PANE_ID="p-self")
        env.pop("HERDR_PLUGIN_CONFIG_DIR", None)
        env.pop("HERDR_SOCKET_PATH", None)
        env["HERDR_CONFIG_PATH"] = str(directory / "missing-herdr-config.toml")
        env["XDG_CONFIG_HOME"] = str(directory)
        env["HOME"] = str(directory)
        child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
        os.close(slave)
        try:
            raw = frame(master, b"  Search: ")
            assert "\x1b[?25l" in raw and "\x1b[?25h" not in raw, raw[-100:]
            _, wraps, cursor = screen(raw, 80, 15, track_cursor=True)
            assert wraps == 0 and cursor[0] is False, cursor
            lines = assert_selected_background(raw, 80, 15)
            selected_raw = next(line for line in raw.splitlines() if "  \x1b[48;5;236m> " in line)
            assert "Claude \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking\x1b[38;2;205;214;244m" in selected_raw, raw
            assert selected_raw.endswith("\x1b[0m\x1b[48;2;24;24;37m\x1b[K"), selected_raw
            assert "codex \x1b[2m|\x1b[22m \x1b[38;2;166;227;161midle\x1b[38;2;205;214;244m" in raw, raw
            assert lines[0] == "  Search: / to search" and not any("Herdr Crossroads" in line for line in lines), lines
            assert lines[1].startswith("  ─") and all(header in lines[2] for header in ("Workspace", "Tab", "Agent/Pane Name")) and "Status" not in lines[2], lines
            assert lines[15-len(default_footer_lines(80))-1].startswith("  ─") and not any("│" in line or "├" in line or "▏" in line for line in lines), lines
            assert sum("┄" in line for line in lines) == 1, lines
            assert lines[8].startswith("  ┄") and "codex" in lines[9], lines
            assert lines[3].startswith("  > ") and "Claude" in lines[3], lines
            assert any("Same" in line and "Claude" in line and "working" in line for line in lines), lines
            assert any("Same" in line and "Claude" not in line and "working" not in line for line in lines[3:8]), lines
            assert not any(pane_id in "\n".join(lines) for pane_id in ("p-b", "p-a", "p-other")), lines
            assert not any("Filter:" in line for line in lines), lines
            assert_footer(lines, 80)
            os.write(master, b"?")
            raw = frame(master, b"Esc: close")
            lines = assert_help_frame(raw)
            assert lines[0] == "  Search: / to search" and lines[2] == "  Navigation", lines
            assert lines[3].startswith("  > j/↓") and "next pane" in lines[3], lines
            assert lines[7].endswith("next/previous workspace") and lines[8] == "" and lines[9] == "  Actions", lines
            _, _, backgrounds = screen(raw, 80, 15, track_bg=True)
            assert [x for x, enabled in enumerate(backgrounds[3]) if enabled] == list(range(2, 78)), backgrounds[3]
            assert not any(backgrounds[8]), backgrounds[8]
            for _ in range(12):
                os.write(master, b"\x1b[B")
                lines = assert_help_frame(frame(master, b"Esc: close"))
            assert any(line.startswith("  > Esc/q") and "close Crossroads" in line for line in lines), lines
            os.write(master, b"\r")
            assert_help_frame(frame(master, b"Esc: close"))
            assert child.poll() is None, "Help Enter focused or closed"
            os.write(master, b"/focus")
            raw = frame(master, b"\x1b[?25h")
            lines = assert_help_frame(raw, searching=True)
            assert lines[0] == "  Search: focus" and lines[2] == "  Actions", lines
            assert not any("Navigation" in line for line in lines), lines
            assert any("focus selected pane" in line for line in lines), lines
            assert lines[3].startswith("  > Enter") and all(not line for line in lines[4:-2]), lines
            assert "\x1b[38;2;180;190;254m\x1b[4m" in raw, raw
            os.write(master, b"\x1b")
            assert_help_frame(frame(master, b"Esc: close"))
            os.write(master, b"r/qr?")
            lines = assert_help_frame(frame(master, b"\x1b[?25h"), searching=True)
            assert lines[0] == "  Search: qr?" and lines[2] == "  No matching keys", lines
            assert not any("Navigation" in line or "Actions" in line for line in lines), lines
            assert all(not line for line in lines[3:-2]), lines
            os.write(master, b"\x7f\r")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert lines[0] == "  Search: qr", lines
            os.write(master, b"r")
            assert_help_frame(frame(master, b"Esc: close"))
            for width, height in ((120, 24), (162, 36), (109, 15), (110, 15), (80, 15)):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                lines = assert_help_frame(frame(master, b"Esc: close"), width, height)
                assert any(line.startswith("  > Enter") for line in lines), lines
                actions = lines.index("  Actions")
                assert lines[actions - 1] == "" and lines[actions - 2].endswith("next/previous workspace"), lines
            for _ in range(12):
                os.write(master, b"k")
                lines = assert_help_frame(frame(master, b"Esc: close"))
            assert lines[3].startswith("  > j/↓"), lines
            os.write(master, b"q")
            assert_selected_background(frame(master, b"  Search: "), 80, 15)
            os.write(master, b"\t")
            lines = assert_selected_background(frame(master, b"  Search: "), 80, 15)
            assert any(line.startswith("  > ") and "codex" in line for line in lines), lines
            os.write(master, b"\x1b[Z")
            lines = assert_selected_background(frame(master, b"  Search: "), 80, 15)
            for width, height, has_rule in ((162, 36, True), (109, 36, False), (110, 36, True), (162, 7, False)):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                if height < 15:
                    raw = frame(master, NOTICE_END)
                    assert [line for line in screen(raw, width, height)[0] if line] == notice_lines(width, height), raw
                    continue
                assert_selected_background(frame(master, b"  Search: "), width, height)
                os.write(master, b"r")
                lines = assert_selected_background(frame(master, b"  Search: "), width, height)
                assert (lines[3] == "  " + "┄" * (width - 4)) == has_rule, (width, height, lines[3])
                assert ("Status" in lines[2]) == (width >= 110), (width, height, lines[2])
            assert any(line.startswith("  > ") and "Claude" in line for line in lines), lines
            for width in (150, 139, 138, 121, 120, 111, 110):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, width, 0, 0))
                assert_selected_background(frame(master, b"  Search: "), width, 15)
                os.write(master, b"r")
                lines = assert_selected_background(frame(master, b"  Search: "), width, 15)
                assert "Status" in lines[2] and lines[3].startswith("  ┄") and "Claude" in lines[4], lines
                assert_footer(lines, width)
            for width in (109, 110):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, width, 0, 0))
                lines = assert_selected_background(frame(master, b"  Search: "), width, 15)
                assert ("Status" in lines[2]) == (width == 110), lines
                assert_footer(lines, width)
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 80, 0, 0))
            raw = frame(master, b"  Search: ")
            assert_selected_background(raw, 80, 15)
            os.write(master, b"/codex\r")
            raw = frame(master, b"Search: codex")
            assert "codex" in "\n".join(screen(raw, 80, 15)[0]), raw
            before_help = screen(raw, 80, 15)[0]
            os.write(master, b"?")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert lines[0] == "  Search: / to search", lines
            os.write(master, b"q")
            assert screen(frame(master, b"  Search: codex"), 80, 15)[0] == before_help
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 110, 0, 0))
            lines = assert_selected_background(frame(master, b"  Search: codex"), 110, 15)
            assert "Status" in lines[2] and lines[3].startswith("  ┄"), lines
            assert any(line.startswith("  > ") and "codex" in line and "idle" in line for line in lines), lines
            assert_footer(lines, 110)
            for width, height in ((79, 15), (80, 14), (78, 14), (67, 12)):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                raw = frame(master, NOTICE_END)
                assert_panel_background(raw, width, height)
                assert_notice_style(raw)
                lines, wraps, cursor = screen(raw, width, height, track_cursor=True)
                assert wraps == 0 and cursor[0] is False, (lines, wraps, cursor)
                assert [line for line in lines if line] == notice_lines(width, height), lines
                assert not any("Search:" in line or "Claude" in line or "focus" in line or "─" in line for line in lines), lines
                for key in (b"/", b"?", b"j", b"k", b"\x1b[A", b"\x1b[B"):
                    os.write(master, key)
                    raw = frame(master, NOTICE_END)
                    assert [line for line in screen(raw, width, height)[0] if line] == notice_lines(width, height), raw
                    assert child.poll() is None, key
            for width, height in ((40, 2), (40, 1)):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                marker = notice_lines(width, height)[0].encode()
                raw = frame(master, marker)
                assert_notice_style(raw)
                assert_panel_background(raw, width, height)
                lines, wraps = screen(raw, width, height)
                assert wraps == 0 and lines == notice_lines(width, height), (lines, wraps)
                for key in (b"j", b"k", b"\x1b[A", b"\x1b[B"):
                    os.write(master, key)
                    raw = frame(master, marker)
                    assert screen(raw, width, height)[0] == lines
                    assert child.poll() is None, key
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 80, 0, 0))
            raw = frame(master, b"Search: codex")
            lines = assert_selected_background(raw, 80, 15)
            assert lines[0] == "  Search: codex" and any(line.startswith("  > ") and "codex" in line for line in lines), lines
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 79, 0, 0))
            raw = frame(master, NOTICE_END)
            assert [line for line in screen(raw, 79, 15)[0] if line] == notice_lines(79, 15)
            os.write(master, b"q")
            raw = frame(master, b"\x1b[?25h")
            assert raw.endswith("\x1b[?25h"), raw[-100:]
            assert child.wait(timeout=3) == 0
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
            os.close(master)

        config = directory / "plugin"
        config.mkdir()
        (config / "theme.json").write_text('{"theme":"unknown"}')
        for width in (80, 120):
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, width, 0, 0))
            notice_env = dict(env, HERDR_PLUGIN_CONFIG_DIR=str(config))
            child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=notice_env)
            os.close(slave)
            try:
                raw = frame(master, b"\x1b[?25l")
                lines, wraps = screen(raw, width, 15)
                notice_row = 15 - len(default_footer_lines(width)) - 1
                assert wraps == 0 and lines[notice_row].strip().startswith("ERROR") and "Theme configuration" in lines[notice_row], lines
                assert "theme.json" not in lines[notice_row] and str(config) not in lines[notice_row], lines
                assert "\x1b[1m\x1b[38;2;243;139;168mERROR" in raw, raw
                assert_footer(lines, width)
                os.write(master, b"t")
                raw = frame(master, b"\x1b[?25l")
                lines, wraps = screen(raw, width, 15)
                assert wraps == 0 and "ERROR" not in "\n".join(lines), lines
                assert_footer(lines, width, "themes")
                os.write(master, b"\x1b")
                frame(master, b"\x1b[?25l")
                os.write(master, b"q")
                frame(master, b"\x1b[?25h")
                assert child.wait(timeout=3) == 0
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
                os.close(master)

        for key in (b"\r", b"\x1b", b"q"):
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 79, 0, 0))
            saved = termios.tcgetattr(slave)
            child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
            try:
                initial = frame(master, NOTICE_END)
                assert "\x1b[?25l" in initial, initial[-100:]
                assert [line for line in screen(initial, 79, 15)[0] if line] == notice_lines(79, 15)
                os.write(master, key)
                restored = frame(master, b"\x1b[?25h")
                assert "\x1b[?25h" in restored, restored[-100:]
                assert child.wait(timeout=3) == 0, key
                assert termios.tcgetattr(slave) == saved, key
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
                os.close(master)
                os.close(slave)


if __name__ == "__main__":
    main(sys.argv[1])
