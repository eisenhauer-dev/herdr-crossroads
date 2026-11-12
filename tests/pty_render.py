"""Linux PTY screen regression for narrow raw-mode popup rendering."""
import errno
import fcntl
import json
import os
import platform
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import unicodedata
from pathlib import Path

from terminal_restoration import raw_terminal_restored


def frame(master, marker):
    data = bytearray()
    while marker not in data:
        ready, _, _ = select.select([master], [], [], 3)
        assert ready, f"no frame ending in {marker!r}: {data[-100:]!r}"
        chunk = os.read(master, 65536)
        assert chunk, f"EOF before frame ending in {marker!r}: {data[-100:]!r}"
        data.extend(chunk)
    while select.select([master], [], [], 0.05)[0]:
        try:
            chunk = os.read(master, 65536)
            if not chunk:
                break
            data.extend(chunk)
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            break
    return data.decode("utf-8", "replace")


def screen(data, width=76, height=12, track_bg=False, track_cursor=False, track_panel_bg=False, track_colors=False, track_italic=False):
    cells = [[" "] * width for _ in range(height)]
    foreground, background = "default-fg", "default-bg"
    reverse = False
    italic = False
    italics = [[False] * width for _ in range(height)] if track_italic else None
    track_style = track_bg or track_panel_bg or track_colors
    colors = [[(foreground, background)] * width for _ in range(height)] if track_style else None
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
                if track_italic:
                    italics = [[italic] * width for _ in range(height)]
                effective = (background, foreground) if reverse else (foreground, background)
                if track_style:
                    colors = [[effective] * width for _ in range(height)]
                # Timed hint pages may precede a resize frame at the previous dimensions.
                # Check wraps in the final full repaint, just like the cells below.
                wraps = 0
            elif command == "K":
                assert params == "", params
                # A full-width write leaves wrap pending with the cursor on the
                # final cell, not beyond it. EL still erases that occupied cell.
                for at in range(min(x, width - 1), width):
                    cells[y][at] = " "
                    if track_italic:
                        italics[y][at] = italic
                    if track_style:
                        colors[y][at] = (background, foreground) if reverse else (foreground, background)
            elif command == "m":
                codes = [int(code or 0) for code in params.split(";")]
                at = 0
                while at < len(codes):
                    code = codes[at]
                    at += 1
                    if code == 0:
                        foreground, background, reverse = "default-fg", "default-bg", False
                        italic = False
                    elif code in (3, 23):
                        italic = code == 3
                    elif code in (7, 27):
                        reverse = code == 7
                    elif code == 39:
                        foreground = "default-fg"
                    elif code == 49:
                        background = "default-bg"
                    elif 30 <= code <= 37 or 90 <= code <= 97:
                        foreground = code - (30 if code < 90 else 82)
                    elif 40 <= code <= 47 or 100 <= code <= 107:
                        background = code - (40 if code < 100 else 92)
                    elif code in (38, 48):
                        kind = codes[at]
                        assert kind in (2, 5), codes
                        count = 3 if kind == 2 else 1
                        color = tuple(codes[at + 1:at + 1 + count]) if kind == 2 else codes[at + 1]
                        at += 1 + count
                        if code == 38:
                            foreground = color
                        else:
                            background = color
            else:
                assert params in ("?25", "?1000", "?1003", "?1006"), (params, command)
                if params == "?25":
                    cursor_visible = command == "h"
            i = end + 1
            continue
        if c == "\r":
            x = 0
        elif c == "\n":
            y = min(y + 1, height - 1)
        else:
            char_width = 1 if c.isascii() else 0 if unicodedata.combining(c) else 2 if unicodedata.east_asian_width(c) in "WF" else 1
            if x >= width:
                wraps += 1
                x = 0
                y = min(y + 1, height - 1)
            if char_width:
                cells[y][x] = c
                if track_italic:
                    for col in range(x, min(width, x + char_width)):
                        italics[y][col] = italic
                if track_style:
                    for col in range(x, min(width, x + char_width)):
                        colors[y][col] = (background, foreground) if reverse else (foreground, background)
                x += char_width
        i += 1
    result = (["".join(line).rstrip() for line in cells], wraps)
    if track_italic:
        return (*result, italics)
    if track_colors:
        return (*result, colors)
    if track_bg:
        return (*result, [[bg == 236 for _, bg in row] for row in colors])
    if track_panel_bg:
        return (*result, [[bg == (24, 24, 37) for _, bg in row] for row in colors])
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
    assert_mode_badge_colors(raw, width, height)
    return lines


def assert_panel_background(raw, width, height):
    lines, wraps, colors = screen(raw, width, height, track_colors=True)
    assert wraps == 0, (width, height, wraps, lines)
    assert_mode_badge_colors(raw, width, height)
    for y, row in enumerate(colors):
        badge = re.search(r"MODE: (SELECT|AGENT \[(all|blocked|done|working|idle|unknown)\]|HELP|THEMES)$", lines[y]) if lines[y].startswith("  Search:") else None
        for x, (_, bg) in enumerate(row):
            expected = (205, 214, 244) if badge and badge.start() <= x < badge.end() else (24, 24, 37)
            assert bg == expected, (x, y, bg, expected, lines)
    return lines


def assert_mode_badge_colors(raw, width, height, foreground=(205, 214, 244), background=(24, 24, 37)):
    lines, wraps, colors = screen(raw, width, height, track_colors=True)
    assert wraps == 0, (width, height, wraps, lines)
    for y, line in enumerate(lines):
        if not line.startswith("  Search:"):
            continue
        badge = re.search(r"MODE: (SELECT|AGENT \[(all|blocked|done|working|idle|unknown)\]|HELP|THEMES)$", line)
        assert badge and badge.end() == width - 2, line
        for x, color in enumerate(colors[y]):
            expected = (background, foreground) if badge.start() <= x < badge.end() else (foreground, background)
            assert color == expected, (x, y, color, expected, line)


def expected_search_row(width, text, mode, agent_filter="all"):
    # These ASCII queries fit without clipping; reserve the renderer's two-cell
    # right margin and assert the entire row, not just the query or badge.
    left = f"  Search: {text}"
    mode = f"MODE: {mode.upper()}"
    if mode == "MODE: AGENT":
        mode += f" [{agent_filter}]"
    assert left.isascii() and len(left) + len(mode) + 1 <= width - 2
    return left + " " * (width - 2 - len(left) - len(mode)) + mode


def assert_help_frame(raw, width=80, height=15, searching=False):
    assert_mode_badge_colors(raw, width, height)
    lines, wraps, cursor = screen(raw, width, height, track_cursor=True)
    assert wraps == 0 and cursor[0] == searching, (lines, wraps, cursor)
    border = "  " + "─" * (width - 4)
    footer = ["  Enter/Esc: return to keys · Backspace: erase"] if searching else default_footer_lines(width, "help")
    assert lines[2] == border and lines[-len(footer) - 1] == border, lines
    assert lines[1] == "  Keys — Select", lines
    assert lines[0].startswith("  Search: ") and lines[0].endswith("MODE: HELP") and len(lines[0]) == width - 2, lines
    assert not any("Keys ·" in line or "Meaning" in line or "Agent/Pane Name" in line for line in lines), lines
    if searching:
        assert lines[-1] == "  Enter/Esc: return to keys · Backspace: erase", lines
        assert "close keys" not in lines[-1], lines
    else:
        assert lines[-len(footer):] == footer, lines
    return lines


NOTICE_END = b"  Press Enter/Esc/q to close."


def assert_terminal_restored(master, slave, saved, active, key, output,
                             pre_close=None, cursor_marker_observed=None):
    actual = termios.tcgetattr(slave)
    if raw_terminal_restored(saved, actual):
        return
    # The cursor marker precedes stty restoration. Collect any later error output
    # only after child.wait(), without waiting or retrying the attribute check.
    tail = bytearray()
    while select.select([master], [], [], 0)[0]:
        try:
            chunk = os.read(master, 65536)
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            break
        if not chunk:
            break
        tail.extend(chunk)
    names = ("iflag", "oflag", "cflag", "lflag", "ispeed", "ospeed", "cc")
    changed = {name: (before, after) for name, before, after in zip(names, saved, actual) if before != after}
    darwin = (f"macos={platform.mac_ver()[0]!r} darwin_kernel={os.uname().release!r} "
              "sdk=unknown (not queried) " if sys.platform == "darwin" else "")
    assert raw_terminal_restored(saved, actual), (f"terminal restoration failed (only Darwin canonical added PENDIN is permitted): key={key!r} "
                             f"platform={sys.platform}/{os.uname().machine} python={sys.version} "
                             f"{darwin}"
                             f"changed={changed!r} saved={saved!r} active={active!r} actual={actual!r} "
                             f"pre_close={pre_close!r} cursor_marker_observed={cursor_marker_observed!r} post_exit={actual!r} "
                             f"output={output!r} post_exit_output={bytes(tail)!r}")


def default_footer_lines(width, mode="select", table=False):
    # Independent expected effective default identities, all visible without paging.
    text = {
        "select": "j/↓: ↓pane · k/↑: ↑pane · h/←: ↑tab · l/→: ↓tab · Tab: ↓ws · Shift+Tab: ↑ws · Enter: focus · Esc: exit · ?: keys · a: agent",
        "help": "j/↓: ↓move · k/↑: ↑move · Enter: run · Esc: close keys",
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


def search_cursor_cases(binary):
    from pty_keymaps import Popup, read_frame, mouse_report

    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-cursor-") as temporary:
        directory = Path(temporary)
        fixture = Path(__file__).parent.resolve() / "fixtures/topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\nif [ \"$*\" = 'api snapshot' ]; then cat '{fixture}'; else echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; fi\n")
        fake.chmod(0o700)
        for width, height in ((80, 15), (142, 30)):
            for context, opening, mode in (
                ("Help", (b"?",), "Help"),
                ("Select", (), "Select"),
                ("Agent", (b"a",), "Agent"),
                ("Themes", (b"t", b" ", mouse_report(4, 1)), "Themes"),
                ("Themes list help", (b"t", b"?"), "Help"),
                ("Themes table help", (b"t", b" ", b"?"), "Help"),
            ):
                badge = f"MODE: {mode.upper()}"
                if mode == "Agent":
                    badge += " [all]"
                budget = width - 4 - len(badge) - 1
                query_budget = budget - 8 - 1  # Keep a cell for the editing cursor.
                wide_count = (query_budget - 1) // 2
                # screen() retains a blank continuation cell after wide glyphs.
                for query, expected_x, visible_query in (
                    ("", 10, ""), ("abc", 13, "abc"), ("abc  ", 15, "abc  "),
                    ("x" * 200, 2 + budget - 1, "x" * query_budget),
                    ("old" * 100 + "LATEST", 2 + budget - 1, ("old" * 100 + "LATEST")[-query_budget:]),
                    ("界" * 100 + "Z", 10 + 2 * wide_count + 1, "界 " * wide_count + "Z"),
                    ("e\u0301" * 200 + "Z", 2 + budget - 1, "e" * (query_budget - 1) + "Z"),
                ):
                    popup = Popup(binary, directory, width=width, height=height)
                    try:
                        for key in opening:
                            popup.send(key)
                        if context != "Themes":
                            popup.send(b"f")
                        expected_row = "  Search: " + visible_query + " " * (width - 2 - expected_x - len(badge)) + badge
                        latest = None

                        def final_query(raw):
                            nonlocal latest
                            # Every draw is a full repaint. Replaying the entire
                            # typing history here is quadratic and can consume the
                            # reader's deadline before the child finishes writing.
                            raw = raw[raw.rfind("\x1b[H\x1b[J"):]
                            lines, wraps, cursor = screen(raw, width, height, track_cursor=True)
                            latest = (lines[0], wraps, cursor)
                            return wraps == 0 and lines[0] == expected_row and cursor == (True, expected_x, 0)

                        # Empty search also needs a complete fresh draw for cursor checks.
                        os.write(popup.master, query.encode() if query else b"\x7f")
                        # Typing can produce many complete intermediate draws. Keep
                        # the popup's frame boundaries and original bounded deadline.
                        try:
                            raw = read_frame(popup.output, expected=final_query)
                        except AssertionError as error:
                            error.add_note(f"width={width} context={context} query={query!r} expected_row={expected_row!r} expected_cursor={(True, expected_x, 0)!r} latest_complete={latest!r}")
                            raise
                        lines, wraps, cursor = screen(raw, width, height, track_cursor=True)
                        assert wraps == 0 and lines[0].endswith(badge), (lines, wraps)
                        assert cursor == (True, expected_x, 0), (width, mode, query, cursor, expected_x, lines[0])
                        assert lines[0] == expected_row, lines
                        if len(query) < 10:
                            assert lines[0] == expected_search_row(width, query, mode), lines
                        elif query.startswith("e\u0301"):
                            assert "Search: " + "e\u0301" * (query_budget - 1) + "Z " in raw, raw
                        assert expected_x < 2 + budget, (expected_x, budget)
                        popup.send(b"\r")
                        if mode == "Help":
                            popup.send(b"q")
                        if opening and opening[0] == b"t":
                            popup.send(b"q")
                        popup.finish(b"q")
                    finally:
                        popup.abort()


def mode_color_cases(binary):
    from pty_keymaps import Popup, read_frame, mouse_report

    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-mode-colors-") as temporary:
        directory = Path(temporary)
        fixture = Path(__file__).parent.resolve() / "fixtures/topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\nif [ \"$*\" = 'api snapshot' ]; then cat '{fixture}'; else echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; fi\n")
        fake.chmod(0o700)
        for theme, foreground, background in (
            ("catppuccin", (205, 214, 244), (24, 24, 37)),
            ("catppuccin-latte", (76, 79, 105), (239, 241, 245)),
            ("terminal", "default-fg", "default-bg"),
        ):
            (directory / "theme.jsonc").write_text(json.dumps({"theme": theme}))
            for width, height in ((80, 15), (142, 30)):
                popup = Popup(binary, directory, width=width, height=height)
                try:
                    assert_mode_badge_colors(popup.initial, width, height, foreground, background)
                    for key, mode, y, query, searching in (
                        (b"f", "SELECT", 0, "", True), (b"\r", "SELECT", 0, "f to search", False),
                        (b"?", "HELP", 0, "f to search", False), (b"f", "HELP", 0, "", True),
                        (b"\r", "HELP", 0, "f to search", False), (b"q", "SELECT", 0, "f to search", False),
                        (b"t", None, 0, "", False), (b" ", "THEMES", 1, "", False),
                        (mouse_report(4, 1), "THEMES", 0, "", True),
                        (b"x", "THEMES", 0, "x", True), (b"\x7f", "THEMES", 0, "", True),
                        (b"\r", "THEMES", 1, "", False),
                        (b"?", "HELP", 0, "f to search", False), (b"f", "HELP", 0, "", True),
                        (b"\r", "HELP", 0, "f to search", False), (b"q", "THEMES", 1, "", False),
                        (b"q", "SELECT", 0, "f to search", False),
                        (b"a", "AGENT", 0, "f to search", False),
                        (b"f", "AGENT", 0, "", True),
                        (b"x", "AGENT", 0, "x", True),
                        (b"\r", "AGENT", 0, "x", False),
                        (b"s", "SELECT", 0, "x", False),
                        (b"r", "SELECT", 0, "f to search", False),
                    ):
                        def expected_mode(raw):
                            # A queued repaint (including the same badge before a
                            # search edit) is not completion of this input action.
                            raw = raw[raw.rfind("\x1b[H\x1b[J"):]
                            lines, wraps, cursor = screen(raw, width, height, track_cursor=True)
                            if mode is None:
                                return lines[0].startswith("  Themes · selected:") and not any("Search:" in line for line in lines) and not cursor[0]
                            return (lines[y] == expected_search_row(width, query, mode)
                                    and (cursor == (True, 10 + len(query), 0) if searching else not cursor[0]))

                        os.write(popup.master, key)
                        raw = read_frame(popup.output, expected=expected_mode)
                        lines, wraps = screen(raw, width, height)
                        assert wraps == 0, lines
                        if mode is None:
                            assert lines[0].startswith("  Themes · selected:") and not any("Search:" in line for line in lines), lines
                        else:
                            assert lines[y] == expected_search_row(width, query, mode), lines
                            assert_mode_badge_colors(raw, width, height, foreground, background)
                    popup.finish(b"q")
                finally:
                    popup.abort()


def launch_selection_cases(binary):
    from pty_keymaps import Popup

    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-launch-") as temporary:
        directory = Path(temporary)
        snapshot = json.loads((Path(__file__).parent / "fixtures/topology.json").read_text())
        topology = snapshot["result"]["snapshot"]
        topology["panes"] = [
            {"pane_id": f"filler-{i}", "workspace_id": "w1", "tab_id": "t1", "label": f"Filler {i:02}"}
            for i in range(30)
        ] + [
            {"pane_id": "caller", "workspace_id": "w2", "tab_id": "t2", "label": "Launch pane"},
            {"pane_id": "next", "workspace_id": "w2", "tab_id": "t2", "label": "Next pane"},
        ]
        topology["agents"] = []
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{directory / 'calls'}'\nif [ \"$*\" = 'api snapshot' ]; then cat '{directory / 'snapshot.json'}'; else echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; fi\n")
        fake.chmod(0o700)
        for empty in (False, True):
            if empty:
                topology["panes"] = []
            (directory / "snapshot.json").write_text(json.dumps(snapshot))
            popup = Popup(binary, directory, launch_context=json.dumps({"focused_pane_id": "caller"}))
            try:
                lines, wraps = screen(popup.initial, 80, 15)
                assert wraps == 0, lines
                selected = [line for line in lines if line.startswith("  >")]
                if empty:
                    assert not selected, lines
                else:
                    assert len(selected) == 1 and "Launch pane" in selected[0], lines
                    assert not any("Filler 00" in line for line in lines), lines
                    shown = "\n".join(lines)
                    assert shown.index("Filler 29") < shown.index("Launch pane"), lines
                    lines = popup.send(b"j")
                    assert any(line.startswith("  >") and "Next pane" in line for line in lines), lines
                    shown = "\n".join(lines)
                    assert shown.index("Launch pane") < shown.index("Next pane"), lines
                popup.finish(b"q")
            finally:
                popup.abort()
        assert (directory / "calls").read_text() == (
            "api snapshot\nworktree list --workspace w1\nworktree list --workspace w2\napi snapshot\n"
        )


def assert_table_header_italics(raw, width, height, agent=False):
    lines, wraps, italics = screen(raw, width, height, track_italic=True)
    assert wraps == 0, lines
    labels = [] if agent else ["Workspace", "Tab", "Agent/Pane Name"] + (["Status"] if width >= 110 else [])
    styled = set()
    for label in labels:
        x = lines[2].index(label)
        styled.update((2, col) for col in range(x, x + len(label)))
    assert {(y, x) for y, row in enumerate(italics) for x, enabled in enumerate(row) if enabled} == styled, (lines, italics)
    assert ("\x1b[3m" in raw and "\x1b[23m" in raw) == (not agent), raw
    if agent:
        assert not any("Agent/Pane Name" in line for line in lines), lines
    return lines


def table_style_and_empty_cases(binary):
    from pty_keymaps import Popup, read_frame, mouse_report

    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-table-") as temporary:
        directory = Path(temporary)
        fixture = json.loads((Path(__file__).parent / "fixtures/topology.json").read_text())
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\nif [ \"$*\" = 'api snapshot' ]; then cat '{directory / 'snapshot.json'}'; else echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; fi\n")
        fake.chmod(0o700)
        config = {"select": {"reset_search": ["R"], "open_agent": ["A"]},
                  "agent": {"reset_search": ["R"], "open_select": ["S"]}}
        for theme in ("catppuccin", "catppuccin-latte", "terminal"):
            (directory / "theme.jsonc").write_text(json.dumps({"theme": theme}))
            for absent in (False, True):
                snapshot = json.loads(json.dumps(fixture))
                if absent:
                    snapshot["result"]["snapshot"]["agents"] = []
                    for pane in snapshot["result"]["snapshot"]["panes"]:
                        pane.pop("agent", None)
                (directory / "snapshot.json").write_text(json.dumps(snapshot))
                popup = Popup(binary, directory, config)
                try:
                    assert_table_header_italics(popup.initial, 80, 15)
                    for agent in (False, True):
                        mode = "AGENT" if agent else "SELECT"
                        if agent:
                            popup.send(b"A")
                        for searching in (True, False):
                            if searching:
                                popup.send(b"f")
                                key = b"zzzz-not-found"
                            else:
                                key = b"\r"
                            os.write(popup.master, key)
                            for width, height in ((80, 15), (150, 30), (110, 15), (80, 15)):
                                if (width, height) != (popup.width, popup.height):
                                    fcntl.ioctl(popup.master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                                    popup.width, popup.height = width, height
                                else:
                                    # Request a fresh complete draw even at unchanged dimensions.
                                    os.write(popup.master, mouse_report(4, 5, 35))

                                def expected(raw):
                                    lines, wraps, cursor = screen(raw, width, height, track_cursor=True)
                                    return (wraps == 0 and lines[0] == expected_search_row(width, "zzzz-not-found", mode)
                                            and cursor[0] == searching and len(lines[1]) == width - 2)

                                raw = read_frame(popup.output, expected=expected)
                                lines = assert_table_header_italics(raw, width, height, agent)
                                footer = ([f"  Enter/Esc: {'agent' if agent else 'select'}  Backspace: erase"] if searching
                                          else default_footer_lines(width))
                                if not searching and not agent:
                                    footer = [line.replace("a: agent", "A: agent") for line in footer]
                                if not searching and agent:
                                    footer = [line.replace("a: agent", "S: select") for line in footer]
                                bottom = height - len(footer) - 1
                                message = "No agent panes" if agent and absent else "No matching agent panes" if agent else "No matching panes"
                                hint = ("Enter/Esc, then " if searching else "")
                                hint += "" if agent and absent else "R: reset search · "
                                hint += "S: select" if agent else "A: agent"
                                y = 4 + (bottom - 4 - 2) // 2
                                expected_body = [""] * (bottom - 4)
                                for offset, text in enumerate((message, hint)):
                                    expected_body[y - 4 + offset] = " " * (2 + (width - 4 - len(text) + 1) // 2) + text
                                assert lines[4:bottom] == expected_body, (lines, expected_body)
                                assert lines[1] == lines[3] == lines[bottom] == "  " + "─" * (width - 4), lines
                                assert lines[-len(footer):] == footer, (lines, footer)
                                assert not any(line.startswith("  >") for line in lines), lines
                                # Empty-body clicks must not focus/close or start search.
                                popup.send(mouse_report(width // 2, y + 1), expected=expected)
                                assert popup.child.poll() is None
                        popup.send(b"R")
                    popup.finish(b"q")
                finally:
                    popup.abort()


def long_empty_preview_hint_case(binary):
    from pty_keymaps import Popup, read_frame, mouse_report

    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-long-hint-") as temporary:
        directory = Path(temporary)
        fixture = Path(__file__).parent.resolve() / "fixtures/topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\nif [ \"$*\" = 'api snapshot' ]; then cat '{fixture}'; else echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; fi\n")
        fake.chmod(0o700)
        config = {"agent": {"reset_search": ["Backspace", "B"], "open_select": ["Space", "S"]},
                  "themes": {"cancel": ["Backspace"]}}
        popup = Popup(binary, directory, config)
        try:
            for key in (b"a", b"t", b" ", mouse_report(4, 1)):
                popup.send(key)
            os.write(popup.master, b"zzzz-not-found")

            def expected(raw):
                lines, wraps, cursor = screen(raw, 80, 15, track_cursor=True)
                return (wraps == 0 and lines[0] == expected_search_row(80, "zzzz-not-found", "THEMES")
                        and cursor == (True, 24, 0))

            raw = read_frame(popup.output, expected=expected)
            lines, wraps = screen(raw, 80, 15)
            hint = "Enter/Esc, then Backspace: table, then Backspace/B: reset search · Space/S: select"
            assert " ".join(line.strip() for line in lines[8:10]) == hint, lines
            body = ["", "", ""] + [" " * (2 + (76 - len(text) + 1) // 2) + text for text in (
                "No matching agent panes", hint.rsplit(" ", 1)[0], "select"
            )] + ["", "", ""]
            assert lines == [expected_search_row(80, "zzzz-not-found", "THEMES"),
                             "  " + "─" * 76,
                              "",
                             "  " + "─" * 76, *body,
                             "  " + "─" * 76,
                             "  Enter/Esc: preview  Backspace: erase"], lines
            assert wraps == 0
            popup.send(b"\r")
            popup.send(b"\x7f")
            popup.finish(b"q")
        finally:
            popup.abort()


def agent_card_cases(binary):
    from pty_keymaps import Popup, read_frame, mouse_report

    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-cards-") as temporary:
        directory = Path(temporary)
        snapshot = json.loads((Path(__file__).parent / "fixtures/topology.json").read_text())
        topology = snapshot["result"]["snapshot"]
        topology["agents"][0]["title"] = "Implement the card layout " * 10
        next(pane for pane in topology["panes"] if pane["pane_id"] == "p-other")["title"] = "Other task"
        for i in range(6):
            topology["panes"].append({"pane_id": f"card-{i}", "terminal_id": f"card-term-{i}",
                "workspace_id": "w2", "tab_id": "t2", "agent": f"Worker {i}",
                "agent_status": "idle", "title": f"Extra task {i}"})
        (directory / "snapshot.json").write_text(json.dumps(snapshot))
        worktrees = json.dumps({"result": {"type": "worktree_list",
            "source": {"repo_key": "repo", "repo_name": "Crossroads"},
            "worktrees": [{"open_workspace_id": "w1", "branch": "develop"},
                          {"open_workspace_id": "w2", "branch": "feature/cards"}]}})
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{directory / 'calls'}'\nif [ \"$*\" = 'api snapshot' ]; then cat '{directory / 'snapshot.json'}'; else printf '%s\\n' '{worktrees}'; fi\n")
        fake.chmod(0o700)
        for theme in ("catppuccin", "catppuccin-latte", "terminal"):
            (directory / "theme.jsonc").write_text(json.dumps({"theme": theme}))
            popup = Popup(binary, directory, args=("--agent",))
            try:
                def cards(raw, selected="Claude", title="Titel: Implement", mode="AGENT", search_y=0,
                          query=None, status="all", searching=False):
                    lines, wraps, italics = screen(raw, popup.width, popup.height, track_italic=True)
                    cursor = screen(raw, popup.width, popup.height, track_cursor=True)[2]
                    text = query if query is not None else "" if mode == "THEMES" else "f to search"
                    return (wraps == 0 and lines[search_y] == expected_search_row(popup.width, text, mode, status)
                            and cursor[0] == searching
                            and any(line.startswith("  >") and selected in line for line in lines)
                            and any(title in line for line in lines)
                            and not any("Agent/Pane Name" in line or "┄" in line for line in lines)
                            and not any(any(row) for row in italics))

                assert cards(popup.initial), popup.initial
                lines, wraps = screen(popup.initial, 80, 15)
                assert "    Same › Same › Same" in lines, lines
                assert "    Repository: Crossroads" in lines and "    Branch: develop" in lines, lines
                assert next(line for line in lines if "Titel:" in line).endswith("…"), lines
                assert "Titel: Other task" not in "\n".join(lines), lines
                codex_y = next(y for y, line in enumerate(lines) if "codex" in line)
                popup.send(mouse_report(5, codex_y + 1, 35),
                           expected=lambda raw: cards(raw, "codex", "Titel: Other task"))
                # An unchanged pointer report must not reselect a different card
                # after the selected card's height changes.
                popup.send(mouse_report(5, codex_y + 1, 35),
                           expected=lambda raw: cards(raw, "codex", "Titel: Other task"))
                popup.send(b"k", expected=cards)
                popup.send(b"t")
                popup.send(b" ", expected=lambda raw: cards(raw, mode="THEMES", search_y=1))
                popup.send(b"q", expected=cards)
                # Wheel movement may hide the expanded selection, but never
                # changes it; keyboard movement restores a whole selected card.
                for first in ("codex", "Worker 0", "Worker 1", "Worker 2", "Worker 3", "Worker 3"):
                    lines = popup.send(mouse_report(5, 5, 65), expected=lambda raw, first=first:
                                       first in screen(raw, 80, 15)[0][2])
                    assert not any(line.startswith("  >") for line in lines), lines
                lines = popup.send(b"j", expected=lambda raw: cards(raw, "codex", "Titel: Other task"))
                assert "    Branch: feature/cards" in lines, lines
                popup.send(b"k", expected=cards)
                for width, height in ((150, 30), (80, 15)):
                    fcntl.ioctl(popup.master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                    popup.width, popup.height = width, height
                    raw = read_frame(popup.output, expected=cards)
                    lines, wraps = screen(raw, width, height)
                    footer = [line.replace("a: agent", "s: select") for line in default_footer_lines(width)]
                    assert lines[-len(footer):] == footer and wraps == 0, lines
                # All metadata fields are searchable only in Agent, with AND
                # status filtering and independent resets across modal returns.
                for query in ("CROSSROADS", "feature/cards", "Other task"):
                    popup.send(b"f")
                    popup.send(query.encode(), expected=lambda raw: cards(raw, "codex" if query != "CROSSROADS" else "Claude",
                               "Titel: Other task" if query != "CROSSROADS" else "Titel: Implement",
                               query=query, searching=True))
                    popup.send(b"\r")
                    lines = popup.send(b"s", expected=lambda raw: "No matching panes" in raw and "MODE: SELECT" in raw)
                    assert not any("Repository:" in line or "Titel:" in line for line in lines), lines
                    popup.send(b"a")
                    popup.send(b"r", expected=lambda raw: cards(raw,
                               "Claude" if query == "CROSSROADS" else "codex",
                               "Titel: Implement" if query == "CROSSROADS" else "Titel: Other task"))
                    if query != "CROSSROADS":
                        popup.send(b"k", expected=cards)
                popup.send(b"i", expected=lambda raw: cards(raw, "codex", "Titel: Other task", status="idle"))
                popup.send(b"f")
                popup.send(b"Other task", expected=lambda raw: cards(raw, "codex", "Titel: Other task",
                           query="Other task", status="idle", searching=True))
                popup.send(b"\r")
                popup.send(b"?", expected=lambda raw: "Keys — Agent" in raw)
                popup.send(b"q", expected=lambda raw: cards(raw, "codex", "Titel: Other task", query="Other task", status="idle"))
                popup.send(b"t")
                popup.send(b" ", expected=lambda raw: cards(raw, "codex", "Titel: Other task", mode="THEMES",
                           search_y=1, query="Other task"))
                popup.send(b"q", expected=lambda raw: cards(raw, "codex", "Titel: Other task", query="Other task", status="idle"))
                popup.send(b"w", expected=lambda raw: "No matching agent panes" in raw and "[working]" in raw)
                popup.send(b"a", expected=lambda raw: cards(raw, "codex", "Titel: Other task", query="Other task"))
                popup.send(b"i")
                lines = popup.send(b"r", expected=lambda raw: cards(raw, "codex", "Titel: Other task", status="idle"))
                assert not any("Claude" in line for line in lines), lines
                popup.send(b"s", expected=lambda raw: "Agent/Pane Name" in raw)
                popup.finish(b"q", expect_mouse=True)
            finally:
                popup.abort()
        assert (directory / "calls").read_text() == (
            "api snapshot\nworktree list --workspace w1\n" * 3
        ), "card navigation must not query metadata"


def main(binary):
    agent_card_cases(binary)
    long_empty_preview_hint_case(binary)
    table_style_and_empty_cases(binary)
    launch_selection_cases(binary)
    search_cursor_cases(binary)
    mode_color_cases(binary)
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-screen-") as path:
        directory = Path(path)
        fixture = Path(__file__).parent / "fixtures" / "topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\ncat '{fixture}'\n")
        fake.chmod(0o700)
        master, slave = pty.openpty()
        env = dict(os.environ, HERDR_BIN_PATH=str(fake), HERDR_PANE_ID="p-self")
        env.pop("HERDR_PLUGIN_CONFIG_DIR", None)
        env.pop("HERDR_SOCKET_PATH", None)
        env["HERDR_CONFIG_PATH"] = str(directory / "missing-herdr-config.toml")
        env["XDG_CONFIG_HOME"] = str(directory)
        env["HOME"] = str(directory)
        child = None
        try:
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 80, 0, 0))
            child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
            raw = frame(master, b"  Search: ")
            assert "\x1b[?25l" in raw and "\x1b[?25h" not in raw, raw[-100:]
            _, wraps, cursor = screen(raw, 80, 15, track_cursor=True)
            assert wraps == 0 and cursor[0] is False, cursor
            lines = assert_selected_background(raw, 80, 15)
            selected_raw = next(line for line in raw.splitlines() if "  \x1b[48;5;236m> " in line)
            assert "Claude \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking\x1b[38;2;205;214;244m" in selected_raw, raw
            assert selected_raw.endswith("\x1b[0m\x1b[48;2;24;24;37m\x1b[K"), selected_raw
            assert "codex \x1b[2m|\x1b[22m \x1b[38;2;166;227;161midle\x1b[38;2;205;214;244m" in raw, raw
            assert lines[0] == expected_search_row(80, "f to search", "Select") and not any("Herdr Crossroads" in line for line in lines), lines
            assert lines[1].startswith("  ─") and all(header in lines[2] for header in ("Workspace", "Tab", "Agent/Pane Name")) and "Status" not in lines[2], lines
            assert lines[15-len(default_footer_lines(80))-1].startswith("  ─") and not any("│" in line or "├" in line or "▏" in line for line in lines), lines
            assert sum("┄" in line for line in lines) == 1, lines
            assert lines[3] == "  " + "─" * 76, lines
            assert lines[9].startswith("  ┄") and "codex" in lines[10], lines
            assert lines[4].startswith("  > ") and "Claude" in lines[4], lines
            assert any("Same" in line and "Claude" in line and "working" in line for line in lines), lines
            assert any("Same" in line and "Claude" not in line and "working" not in line for line in lines[3:8]), lines
            assert not any(pane_id in "\n".join(lines) for pane_id in ("p-b", "p-a", "p-other")), lines
            assert not any("Filter:" in line for line in lines), lines
            assert_footer(lines, 80)
            os.write(master, b"?")
            raw = frame(master, b"Esc: close")
            lines = assert_help_frame(raw)
            assert lines[0] == expected_search_row(80, "f to search", "Help") and lines[3] == "  Navigation", lines
            assert lines[4].startswith("  > j/↓") and "next pane" in lines[4], lines
            assert lines[8].endswith("next workspace") and lines[9].endswith("previous workspace") and lines[10] == "" and lines[11] == "  Actions", lines
            _, _, backgrounds = screen(raw, 80, 15, track_bg=True)
            assert [x for x, enabled in enumerate(backgrounds[4]) if enabled] == list(range(2, 78)), backgrounds[4]
            assert not any(backgrounds[10]), backgrounds[10]
            for keys, meaning in (
                ("k/↑", "previous pane"), ("h/←", "previous tab"),
                ("l/→", "next tab"), ("Tab", "next workspace"),
                ("Shift+Tab", "previous workspace"), ("Enter", "focus selected pane"),
                ("f", "search"), ("r", "reset search"), ("t", "choose theme"),
                ("?", "show keys"), ("Esc/q", "close Crossroads"),
            ):
                os.write(master, b"\x1b[B")
                lines = assert_help_frame(frame(master, b"Esc: close"))
                selected = [line for line in lines if line.startswith("  > ")]
                assert len(selected) == 1 and selected[0].startswith(f"  > {keys} ") and selected[0].endswith(meaning), lines
            assert any(line.startswith("  > Esc/q") and "close Crossroads" in line for line in lines), lines
            os.write(master, b"\x1b[B")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert any(line.startswith("  > a ") and line.endswith("show agents") for line in lines), lines
            for keys, meaning in (
                ("Hover", "highlight row; theme preview only, never activate"),
                ("Left click", "focus clicked pane; Help runs action; footer or search"),
                ("Wheel", "scroll viewport only; keep selection, including search"),
            ):
                os.write(master, b"\x1b[B")
                lines = assert_help_frame(frame(master, b"Esc: close"))
                assert "  Mouse (fixed)" in lines, lines
                selected = [line for line in lines if line.startswith("  > ")]
                assert len(selected) == 1 and selected[0].startswith(f"  > {keys} ") and selected[0].endswith(meaning), lines
                os.write(master, b"\r")
                assert assert_help_frame(frame(master, b"Esc: close")) == lines, lines
                assert child.poll() is None, f"Enter activated informational {keys} row"
            os.write(master, b"\x1b[B")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert lines[3] == "  Navigation" and lines[4].startswith("  > j/↓"), lines
            os.write(master, b"\x1b[A")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert any(line.startswith("  > Wheel") and "scroll viewport" in line for line in lines), lines
            os.write(master, b"\x1b[106;5u")
            assert_help_frame(frame(master, b"Esc: close"))
            assert child.poll() is None, "Ctrl+j in Help focused or closed"
            os.write(master, b"ffocus")
            raw = frame(master, b"\x1b[?25h")
            lines = assert_help_frame(raw, searching=True)
            assert lines[0] == expected_search_row(80, "focus", "Help") and lines[3] == "  Actions", lines
            assert not any("Navigation" in line for line in lines), lines
            assert any("focus selected pane" in line for line in lines), lines
            assert lines[4].startswith("  > Enter") and lines[5] == "" and lines[6] == "  Mouse (fixed)", lines
            assert lines[7].startswith("    Left click") and "focus clicked pane" in lines[7], lines
            assert all(not line for line in lines[8:-2]), lines
            assert "\x1b[38;2;180;190;254m\x1b[4m" in raw, raw
            os.write(master, b"\x1b")
            assert_help_frame(frame(master, b"Esc: close"))
            os.write(master, b"rfqr?")
            lines = assert_help_frame(frame(master, b"\x1b[?25h"), searching=True)
            assert lines[0] == expected_search_row(80, "qr?", "Help") and lines[3] == "  No matching keys", lines
            assert not any("Navigation" in line or "Actions" in line for line in lines), lines
            assert all(not line for line in lines[4:-2]), lines
            os.write(master, b"\x7f\r")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert lines[0] == expected_search_row(80, "qr", "Help"), lines
            os.write(master, b"r")
            assert_help_frame(frame(master, b"Esc: close"))
            for width, height in ((120, 24), (162, 36), (109, 15), (110, 15), (80, 15)):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                lines = assert_help_frame(frame(master, b"Esc: close"), width, height)
                assert any(line.startswith("  > Enter") for line in lines), lines
                actions = lines.index("  Actions")
                assert lines[actions - 1] == "" and lines[actions - 2].endswith("previous workspace"), lines
            for _ in range(6):
                os.write(master, b"k")
                lines = assert_help_frame(frame(master, b"Esc: close"))
            assert lines[4].startswith("  > j/↓"), lines
            os.write(master, b"k")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert any(line.startswith("  > Wheel") and "scroll viewport" in line for line in lines), lines
            os.write(master, b"j")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert lines[3] == "  Navigation" and lines[4].startswith("  > j/↓"), lines
            os.write(master, b"q")
            assert_selected_background(frame(master, b"  Search: "), 80, 15)
            os.write(master, b"\t")
            lines = assert_selected_background(frame(master, b"  Search: "), 80, 15)
            assert any(line.startswith("  > ") and "codex" in line for line in lines), lines
            os.write(master, b"\x1b[Z")
            lines = assert_selected_background(frame(master, b"  Search: "), 80, 15)
            for width, height in ((162, 36), (109, 36), (110, 36), (162, 7)):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                if height < 15:
                    raw = frame(master, NOTICE_END)
                    assert [line for line in screen(raw, width, height)[0] if line] == notice_lines(width, height), raw
                    continue
                assert_selected_background(frame(master, b"  Search: "), width, height)
                os.write(master, b"r")
                lines = assert_selected_background(frame(master, b"  Search: "), width, height)
                assert lines[3] == "  " + "─" * (width - 4), (width, height, lines[3])
                assert ("Status" in lines[2]) == (width >= 110), (width, height, lines[2])
            assert any(line.startswith("  > ") and "Claude" in line for line in lines), lines
            for width in (150, 139, 138, 121, 120, 111, 110):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, width, 0, 0))
                assert_selected_background(frame(master, b"  Search: "), width, 15)
                os.write(master, b"r")
                lines = assert_selected_background(frame(master, b"  Search: "), width, 15)
                assert "Status" in lines[2] and lines[3] == "  " + "─" * (width - 4) and "Claude" in lines[4], lines
                assert_footer(lines, width)
            for width in (109, 110):
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, width, 0, 0))
                lines = assert_selected_background(frame(master, b"  Search: "), width, 15)
                assert ("Status" in lines[2]) == (width == 110), lines
                assert_footer(lines, width)
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 80, 0, 0))
            raw = frame(master, b"  Search: ")
            assert_selected_background(raw, 80, 15)
            os.write(master, b"fcodex\r")
            raw = frame(master, b"Search: codex")
            assert "codex" in "\n".join(screen(raw, 80, 15)[0]), raw
            before_help = screen(raw, 80, 15)[0]
            os.write(master, b"?")
            lines = assert_help_frame(frame(master, b"Esc: close"))
            assert lines[0] == expected_search_row(80, "f to search", "Help"), lines
            os.write(master, b"q")
            assert screen(frame(master, b"  Search: codex"), 80, 15)[0] == before_help
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 110, 0, 0))
            lines = assert_selected_background(frame(master, b"  Search: codex"), 110, 15)
            assert "Status" in lines[2] and lines[3] == "  " + "─" * 106, lines
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
                for key in (b"f", b"?", b"j", b"k", b"\x1b[A", b"\x1b[B"):
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
            assert lines[0] == expected_search_row(80, "codex", "Select") and any(line.startswith("  > ") and "codex" in line for line in lines), lines
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 79, 0, 0))
            raw = frame(master, NOTICE_END)
            assert [line for line in screen(raw, 79, 15)[0] if line] == notice_lines(79, 15)
            os.write(master, b"q")
            raw = frame(master, b"\x1b[?25h")
            assert raw.endswith("\x1b[?25h"), raw[-100:]
            assert child.wait(timeout=3) == 0
        finally:
            if child is not None and child.poll() is None:
                child.kill()
                child.wait()
            os.close(master)
            os.close(slave)

        config = directory / "plugin"
        config.mkdir()
        (config / "theme.json").write_text('{"theme":"unknown"}')
        for width in (80, 120):
            master, slave = pty.openpty()
            notice_env = dict(env, HERDR_PLUGIN_CONFIG_DIR=str(config))
            child = None
            try:
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, width, 0, 0))
                child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=notice_env)
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
                if child is not None and child.poll() is None:
                    child.kill()
                    child.wait()
                os.close(master)
                os.close(slave)

        for key in (b"\r", b"\x1b", b"q"):
            master, slave = pty.openpty()
            child = None
            try:
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 15, 79, 0, 0))
                saved = termios.tcgetattr(slave)
                child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
                initial = frame(master, NOTICE_END)
                assert "\x1b[?25l" in initial, initial[-100:]
                assert [line for line in screen(initial, 79, 15)[0] if line] == notice_lines(79, 15)
                active = termios.tcgetattr(slave)
                os.write(master, key)
                restored = frame(master, b"\x1b[?25h")
                assert "\x1b[?25h" in restored, restored[-100:]
                # Observation after frame() returns, not stty completion or the
                # instant the marker was emitted; frame() also drains output.
                cursor_marker_observed = termios.tcgetattr(slave) if sys.platform == "darwin" else None
                assert child.wait(timeout=3) == 0, key
                assert_terminal_restored(master, slave, saved, active, key, initial + restored,
                                         pre_close=active if sys.platform == "darwin" else None,
                                         cursor_marker_observed=cursor_marker_observed)
            finally:
                if child is not None and child.poll() is None:
                    child.kill()
                    child.wait()
                os.close(master)
                os.close(slave)


if __name__ == "__main__":
    main(sys.argv[1])
