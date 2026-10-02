"""Linux PTY regression for per-key redraw latency; invoked by startup.rs."""
import json
import os
import pty
import re
import select
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def read_frame(master, ending, timeout=5):
    data = bytearray()
    deadline = time.monotonic() + timeout
    while ending not in data:
        remaining = deadline - time.monotonic()
        assert remaining > 0, f"timed out waiting for {ending!r}; tail={data[-200:]!r}"
        ready, _, _ = select.select([master], [], [], remaining)
        assert ready, f"timed out waiting for {ending!r}; tail={data[-300:]!r}"
        data.extend(os.read(master, 65536))
    return data.decode("utf-8", "replace")


def main(binary):
    panes = [
        {"pane_id": f"p-{i:04}", "workspace_id": "w", "tab_id": "t", "label": "filler"}
        for i in range(997)
    ]
    panes += [
        {"pane_id": "p-x", "workspace_id": "w", "tab_id": "t", "label": "x first"},
        {"pane_id": "p-xy", "workspace_id": "w", "tab_id": "t", "label": "xy middle"},
        {"pane_id": "p-xyz", "workspace_id": "w", "tab_id": "t", "label": "xyz last"},
    ]
    snapshot = {"result": {"type": "session_snapshot", "snapshot": {
        "workspaces": [{"workspace_id": "w", "number": 1, "label": "Workspace"}],
        "tabs": [{"tab_id": "t", "workspace_id": "w", "number": 1, "label": "Tab"}],
        "panes": panes, "agents": []}}}

    with tempfile.TemporaryDirectory(prefix="crossroads-pty-", dir="/tmp/opencode") as directory:
        directory = Path(directory)
        (directory / "snapshot.json").write_text(json.dumps(snapshot))
        fake = directory / "fake-herdr"
        fake.write_text("#!/bin/sh\nif [ \"$1 $2\" = 'worktree list' ]; then echo '{\"result\":{\"type\":\"worktree_list\",\"worktrees\":[]}}'; exit; fi\n"
                        "printf '%s\\n' \"$1 $2\" >> \"$CROSSROADS_TEST_LOG\"\n"
                        "[ \"$1 $2\" = 'api snapshot' ] || exit 9\n"
                        "cat \"$CROSSROADS_TEST_SNAPSHOT\"\n")
        fake.chmod(0o700)
        master, slave = pty.openpty()
        child = None
        try:
            env = dict(os.environ, HERDR_BIN_PATH=str(fake),
                       CROSSROADS_TEST_LOG=str(directory / "calls.log"),
                       CROSSROADS_TEST_SNAPSHOT=str(directory / "snapshot.json"),
                       HERDR_PANE_ID="not-present")
            env.pop("HERDR_SOCKET_PATH", None)
            child = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=env)
            os.close(slave)
            slave = -1
            initial = read_frame(master, b"\x1b[?25l")
            assert "Workspace" in initial and "Tab" in initial and "filler" in initial and "p-0000" not in initial, initial[-300:]
            for key, count in (("x", 3), ("y", 2), ("z", 1)):
                if key == "x":
                    os.write(master, b"/")
                    read_frame(master, b"Backspace: erase")
                start = time.monotonic()
                os.write(master, key.encode())
                query = {"x": "x", "y": "xy", "z": "xyz"}[key]
                frame = read_frame(master, b"Backspace: erase")
                elapsed_ms = (time.monotonic() - start) * 1000
                assert f"Search: {query}" in frame, frame[-300:]
                assert "\x1b[4m" in frame, frame[-300:]
                plain = re.sub(r"\x1b\[[0-9;]*m", "", frame)
                observed = sum(name in plain for name in ("x first", "xy middle", "xyz last"))
                assert all(f"p-{suffix}" not in frame for suffix in ("x", "xy", "xyz")), frame[-300:]
                print(f"1000 rows: key={key}, matches={observed}, keypress-to-view={elapsed_ms:.2f} ms", flush=True)
                assert observed == count, f"query {query}: expected {count} rows, got {observed}"
                assert elapsed_ms <= 100, f"query {query}: {elapsed_ms:.2f} ms > 100 ms"
            os.write(master, b"\x1b")
            # Raw terminal Esc is resolved after the 100ms input timeout.
            read_frame(master, b"\x1b[?25l")
            os.write(master, b"\x1b")
            assert child.wait(timeout=3) == 0, "Esc in Select did not close"
            assert (directory / "calls.log").read_text() == "api snapshot\n"
        finally:
            if child is not None and child.poll() is None:
                child.kill()
                child.wait()
            os.close(master)
            if slave != -1:
                os.close(slave)


if __name__ == "__main__":
    main(sys.argv[1])
