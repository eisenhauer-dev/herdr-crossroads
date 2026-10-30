"""Shared current-cell VT reader for isolated native acceptance fixtures."""
import codecs
import re
import unicodedata


def table_state(screen):
    """Read the current fixture table, not labels from earlier paints."""
    lines = screen.text().splitlines()
    prompt = re.fullmatch(r"  Search: (.*?) +(Select|Search) *", lines[0])
    header = next((y for y, line in enumerate(lines) if "Agent/Pane Name" in line), None)
    if prompt is None or header is None:
        return None
    name_start = lines[header].index("Agent/Pane Name")
    name_end = lines[header].find("Status")
    name_end = name_end - 1 if name_end >= 0 else screen.cols
    bottom = next((y for y in range(header + 2, len(lines))
                   if lines[y].lstrip().startswith("─")), None)
    if bottom is None:
        return None
    panes, marked = [], []
    for line in lines[header + 2:bottom]:
        if not line.strip() or line.lstrip().startswith("┄"):
            continue
        name = line[name_start:name_end].strip()
        if name:
            if line.startswith("  > "):
                marked.append((len(panes), name))
            panes.append(name)
    return {"mode": prompt[2], "query": prompt[1], "panes": tuple(panes),
            "selected": marked[0] if len(marked) == 1 else None,
            # ConPTY's translated delivery state, not an assertion that inner
            # 1000/1006 requests survive as identical outer escape bytes.
            "capture": any(screen.modes.get(mode, False) for mode in (1000, 1002, 1003))}


class Screen:
    """Small current-cell reader for native cursor/erase output, not a transcript.

    Only ASCII assertion labels are used. Unsupported screen-changing controls
    fail rather than silently producing evidence from historical output.
    """

    def __init__(self, rows=40, cols=160):
        self.rows, self.cols = rows, cols
        self.cells = [[" "] * cols for _ in range(rows)]
        self.y = self.x = 0
        self.pending = ""
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.modes = {}

    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            text = self.pending
            if text.startswith("\x1b["):
                match = re.match(r'\x1b\[([0-?]*)([ -/]*)([@-~])', text)
                if not match:
                    break
                raw, intermediate, code = match.groups()
                self.pending = text[match.end():]
                if code == "c" and not intermediate and raw in ["", "0", ">", ">0"]:
                    continue  # Device-attribute queries do not paint cells.
                if code == "n" and not intermediate and raw in ["5", "6", "?996"]:
                    continue  # Status/cursor/theme queries do not paint cells.
                if code == "t" and not intermediate and raw in ["14", "16", "18", "19"]:
                    continue  # Terminal-size queries do not paint cells.
                if code == "u" and not intermediate and re.fullmatch(r"[?<>=][0-9;]*", raw):
                    continue  # Kitty keyboard negotiation does not paint cells.
                if code == "p" and intermediate == "$" and raw == "?1016":
                    continue  # Pixel-mouse mode query does not paint cells.
                if code in "hl" and not intermediate and raw.startswith("?"):
                    for mode in raw[1:].split(";"):
                        self.modes[int(mode)] = code == "h"
                if code in "mhl" or code == "q" and intermediate == " ":
                    continue  # Colors, terminal modes, cursor style do not paint cells.
                assert not intermediate and not raw.startswith(("?", ">", "=")), match.group()
                args = [int(part or "0") for part in raw.split(";")]
                n = args[0] or 1
                if code in "Hf":
                    self.y = min(self.rows - 1, (args[0] or 1) - 1)
                    self.x = min(self.cols - 1, ((args[1] or 1) if len(args) > 1 else 1) - 1)
                elif code == "G":
                    self.x = min(self.cols - 1, n - 1)
                elif code in "ABCD":
                    self.y = max(0, min(self.rows - 1, self.y + (n if code == "B" else -n if code == "A" else 0)))
                    self.x = max(0, min(self.cols - 1, self.x + (n if code == "C" else -n if code == "D" else 0)))
                elif code == "J":
                    assert args[0] in [0, 1, 2, 3]
                    for row in range(self.rows):
                        for col in range(self.cols):
                            if args[0] in [2, 3] or (row, col) >= (self.y, self.x) and args[0] == 0 or (row, col) <= (self.y, self.x) and args[0] == 1:
                                self.cells[row][col] = " "
                elif code == "K":
                    assert args[0] in [0, 1, 2]
                    for col in range(self.cols):
                        if args[0] == 2 or col >= self.x and args[0] == 0 or col <= self.x and args[0] == 1:
                            self.cells[self.y][col] = " "
                elif code == "X":
                    start = min(self.x, self.cols - 1)
                    for col in range(start, min(self.cols, start + n)):
                        self.cells[self.y][col] = " "
                else:
                    raise AssertionError(f"unsupported native CSI: {match.group()!r}")
            elif text.startswith("\x1b]"):
                end = re.search(r'\x07|\x1b\\', text)
                if not end:
                    break
                self.pending = text[end.end():]  # OSC titles/colors, not screen cells.
            elif text[0] == "\x1b":
                if len(text) == 1:
                    break
                assert text[1] in "=>", f"unsupported native escape {text[:2]!r}"
                self.pending = text[2:]
            else:
                char = text[0]
                self.pending = text[1:]
                if char == "\r":
                    self.x = 0
                elif char == "\n":
                    if self.y == self.rows - 1:
                        self.cells.pop(0)
                        self.cells.append([" "] * self.cols)
                    else:
                        self.y += 1
                elif char == "\b":
                    self.x = max(0, self.x - 1)
                elif char == "\t":
                    self.x = min(self.cols - 1, (self.x // 8 + 1) * 8)
                elif ord(char) >= 32 and not unicodedata.combining(char):
                    if self.x >= self.cols:
                        self.x = 0
                        self.y = min(self.rows - 1, self.y + 1)
                    self.cells[self.y][self.x] = char
                    self.x += 2 if unicodedata.east_asian_width(char) in "WF" else 1

    def text(self):
        return "\n".join("".join(row) for row in self.cells)

    def popup_view(self):
        # The fixture has only three root shells (including the host's initial
        # workspace), so its complete pane rows fit in these first 12 table lines.
        # Crop by the actual query/badge coordinates, excluding underlying host
        # chrome and labels rather than claiming their text belongs to the popup.
        lines = self.text().splitlines()
        prompts = [(y, re.search(r'Search: .*?\s+(Select|Search)\b', line))
                   for y, line in enumerate(lines)]
        prompts = [(y, match) for y, match in prompts if match]
        if len(prompts) != 1:
            return ""
        y, match = prompts[0]
        return "\n".join(line[match.start():match.end()] for line in lines[y:y + 12])
