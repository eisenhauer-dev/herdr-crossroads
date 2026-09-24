"""Linux PTY screen regression for narrow raw-mode popup rendering."""
import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
from pathlib import Path


def frame(master, marker):
    data = bytearray()
    while marker not in data:
        ready, _, _ = select.select([master], [], [], 3)
        assert ready, f"no frame ending in {marker!r}: {data[-100:]!r}"
        data.extend(os.read(master, 65536))
    while select.select([master], [], [], 0.05)[0]:
        data.extend(os.read(master, 65536))
    return data.decode("utf-8", "replace")


def screen(data, width=76, height=12):
    cells = [[" "] * width for _ in range(height)]
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
            assert command in "HJ", command
            if command == "H":
                x = y = 0
            else:
                cells = [[" "] * width for _ in range(height)]
            i = end + 1
            continue
        if c == "\r":
            x = 0
        elif c == "\n":
            y = min(y + 1, height - 1)
        else:
            if x >= width:
                wraps += 1
                x = 0
                y = min(y + 1, height - 1)
            cells[y][x] = c
            x += 1
        i += 1
    return ["".join(line).rstrip() for line in cells], wraps


def main(binary):
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-screen-") as path:
        directory = Path(path)
        fixture = Path(__file__).parent / "fixtures" / "topology.json"
        fake = directory / "herdr"
        fake.write_text(f"#!/bin/sh\ncat '{fixture}'\n")
        fake.chmod(0o700)
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 12, 76, 0, 0))
        env = dict(os.environ, HERDR_BIN_PATH=str(fake), HERDR_PANE_ID="p-self")
        child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
        os.close(slave)
        try:
            lines, wraps = screen(frame(master, b"Filter: "))
            assert wraps == 0, f"wrapped {wraps} times: {lines}"
            assert lines[0] == "Herdr Crossroads", lines
            assert lines[1].startswith("> W1 T1 p-b | Claude | working"), lines
            assert lines[2].startswith("  W1 T1 p-a | - | idle"), lines
            assert any("Filter:" in line for line in lines), lines
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 5, 76, 0, 0))
            os.write(master, b"jjjjj")
            lines, wraps = screen(frame(master, b"Filter: "), height=5)
            assert wraps == 0, lines
            assert any(line.startswith("> W2 T1 p-other | codex | idle") for line in lines), lines
            os.write(master, b"/?")
            lines, wraps = screen(frame(master, b"Search: ?"), height=5)
            assert wraps == 0, lines
            assert any("Search: ?" in line for line in lines), lines
            os.write(master, b"\x1b")
            frame(master, b"Filter: ")
            os.write(master, b"q")
            assert child.wait(timeout=3) == 0
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
            os.close(master)


if __name__ == "__main__":
    main(sys.argv[1])
