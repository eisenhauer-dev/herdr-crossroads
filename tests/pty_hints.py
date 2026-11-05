"""Isolated footer stability, grouping and two-key PTY regressions."""
import os
import select
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
from pty_keymaps import Popup
from pty_render import screen

DEFAULT_SELECT = "j/↓: ↓pane · k/↑: ↑pane · h/←: ↑tab · l/→: ↓tab · Tab: ↓ws · Shift+Tab: ↑ws · Enter: focus · Esc: exit · ?: keys"
DEFAULT_HELP = "j/↓: ↓move · k/↑: ↑move · Enter: run · Esc: close keys"
DEFAULT_THEMES = "j/↓: ↓theme · k/↑: ↑theme · Space: table · Enter: save · Esc: cancel"


def normalized(lines):
    return " · ".join(line.strip() for line in lines if line.strip())


def check_frame(raw, width):
    lines, wraps = screen(raw, width, 15)
    assert wraps == 0, (width, lines)
    assert sum(line.startswith("  >") for line in lines) == 1, lines
    return lines


def main(binary):
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-hints-") as temporary:
        directory = Path(temporary)
        fixture = Path(__file__).parent.resolve() / "fixtures/topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\nif [ \"$*\" = 'api snapshot' ]; then cat '{fixture}'; else echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; fi\n")
        fake.chmod(0o700)
        popup = None
        try:
            for width, expected_rows in [(80, 2), (110, 2), (120, 2), (145, 1), (240, 1)]:
                for notice in [False, True]:
                    theme = directory / "theme.jsonc"
                    if notice:
                        theme.write_text("{")
                    elif theme.exists():
                        theme.unlink()
                    popup = Popup(binary, directory, width=width)
                    lines = check_frame(popup.initial, width)
                    assert normalized(lines[-expected_rows:]) == DEFAULT_SELECT, (width, lines)
                    assert any("Claude" in line and "working" in line for line in lines), lines
                    if notice:
                        assert lines[-expected_rows - 1].strip().startswith("ERROR") and "Theme configuration" in lines[-expected_rows - 1], lines
                        assert "theme.json" not in lines[-expected_rows - 1] and str(directory) not in lines[-expected_rows - 1], lines
                    else:
                        assert lines[-expected_rows - 1].startswith("  ─"), lines
                    # Beyond the automatic hint interval there must be no unsolicited repaint.
                    assert not select.select([popup.master], [], [], 2.15)[0], "default hints must remain complete and stable"
                    if not notice:
                        assert popup.send(b"r") == lines, "default repaint changed without a state change"
                        for key, expected, rows in [(b"?", DEFAULT_HELP, 1),
                                                    (b"t", DEFAULT_THEMES, 2),
                                                    (b" ", DEFAULT_THEMES.replace("Space: table", "Space: list"), 2)]:
                            context_lines = popup.send(key)
                            assert normalized(context_lines[-rows:]) == expected, (width, context_lines)
                            assert not select.select([popup.master], [], [], 2.15)[0], "default context hints must stay stable"
                            if key == b"?":
                                popup.send(b"q")
                        popup.send(b"\x1b")
                    popup.finish(b"q")

            for content in [r'{"theme":"\u001b[2J\n\r\t\u0000bad"}',
                            r'{"theme":"dracula","\u001b[2J\n\u007fbad":true}']:
                (directory / "theme.jsonc").write_text(content)
                popup = Popup(binary, directory, width=120)
                assert "\x1b[2J" not in popup.initial, repr(popup.initial)
                lines = check_frame(popup.initial, 120)
                assert len(lines) == 15 and "Claude" in "\n".join(lines), lines
                notice = lines[-3]
                assert notice.strip().startswith("ERROR") and "; using Mocha" in notice, lines
                assert not any(char in notice for char in "\n\r\t\x00\x7f\x1b"), repr(notice)
                assert "theme.json" not in notice and str(directory) not in notice, lines
                assert normalized(lines[-2:]) == DEFAULT_SELECT, lines
                popup.finish(b"q")

            (directory / "theme.jsonc").unlink()
            aliases = ["/", "Down"]
            previous_aliases = ["K", "Up"]
            config = {"select": {"next_row": aliases, "previous_row": previous_aliases, "open_search": ["s"]},
                      "help": {"next_category": aliases, "previous_category": previous_aliases, "open_search": ["s"]},
                      "themes": {"next_theme": aliases, "previous_theme": previous_aliases}}
            for context, open_key, final_hint in [("select", None, "?: keys"), ("help", b"?", "close keys"), ("themes", b"t", "Esc: cancel"), ("preview", b"t", "Esc: cancel")]:
                popup = Popup(binary, directory, config)
                lines = check_frame(popup.initial, 80)
                if open_key:
                    lines = popup.send(open_key)
                if context in ["themes", "preview"]:
                    assert lines[0].startswith("  Themes · selected:") and "Space: table" in normalized(lines[-2:]), lines
                    assert not any("Agent/Pane Name" in line for line in lines), lines
                if context == "preview":
                    lines = popup.send(b" ")
                    assert lines[0].startswith("  Themes · saved:") and "Space: list" in normalized(lines[-2:]), lines
                    assert any("Agent/Pane Name" in line for line in lines), lines
                    assert any("Claude" in line and "working" in line for line in lines), lines
                rows = 2 if context in ["select", "themes", "preview"] else 1
                observed = normalized(lines[-rows:])
                assert all(alias in observed for alias in ["'/'/↓", "K/↑"]) and final_hint in observed, (context, observed)
                assert " · " in observed and " | " not in observed, (context, observed)
                action = "pane" if context == "select" else "move" if context == "help" else "theme"
                assert f"'/'/↓: ↓{action}" in observed, (context, observed)
                for unit in observed.split(" · "):
                    assert any(unit in line for line in lines[-rows:]), (context, unit, lines)
                assert not select.select([popup.master], [], [], 2.15)[0], "valid two-key hints must remain stable"
                if context != "select":
                    popup.send(b"q" if context == "help" else b"\x1b")
                popup.finish(b"q")
        finally:
            if popup:
                popup.abort()
    print("Footer hints PTY: stable defaults, sanitized notices, atomic units, quoted slash and configured arrows passed")


if __name__ == "__main__":
    main(sys.argv[1])
