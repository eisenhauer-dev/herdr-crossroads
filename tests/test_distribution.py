"""Main-owned opt-in native Herdr 0.9.3 distribution acceptance; not discovery/CI.

Contracts: herdrdev/herdr tag v0.9.3 src/session.rs:161-184,
src/app/api/session.rs:16-60, src/app/api/plugins/panes.rs:11-41,
src/app/popup.rs:116-117,273-289 and src/cli/plugin.rs.
No fake server, public downloads, personal session, or Rust/Cargo is used.
"""

import argparse
import codecs
import errno
import fcntl
import hashlib
import io
import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import signal
import socket
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import termios
import time
import unicodedata


ROOT = Path(__file__).resolve().parents[1]
SOURCE = "SimonEisenhauer/herdr-crossroads"
ARCHIVE = "herdr-crossroads-{}-x86_64-unknown-linux-gnu.tar.gz"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def prepare_fixture(base, asset_dir):
    """Copy original artifacts unchanged; create only a test-only trailer variant."""
    repo, assets = base / "source", base / "assets"
    (repo / "scripts").mkdir(parents=True)
    assets.mkdir()
    for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
        shutil.copy2(ROOT / name, repo / name)
    for name in ["check-version.py", "install-binary.py"]:
        shutil.copy2(ROOT / "scripts" / name, repo / "scripts" / name)
    # Dimensions belong only to the temporary fixture, not the project manifest.
    manifest = repo / "herdr-plugin.toml"
    manifest.write_text(manifest.read_text() + '\nwidth = "90%"\nheight = "90%"\n')
    name = ARCHIVE.format("0.1.0")
    for filename in [name, name + ".sha256"]:
        shutil.copy2(asset_dir / filename, assets / filename)
    archive = assets / name
    assert (assets / (name + ".sha256")).read_text() == f"{digest(archive.read_bytes())}  {name}\n"
    with tarfile.open(archive) as package:
        members = package.getmembers()
        assert len(members) == 2 and {m.name for m in members} == {
            "herdr-crossroads", "herdr-crossroads.sha256"}
        assert all(m.isfile() and not m.sparse for m in members)
        original = package.extractfile("herdr-crossroads").read()
        assert package.extractfile("herdr-crossroads.sha256").read() == (
            f"{digest(original)}  herdr-crossroads\n".encode())
    assert original[:4] == b"\x7fELF"
    # Same validated runtime, NOT a newly built/released version or update feature.
    variant = original + b"\nCrossroads test-only distribution fixture 0.1.1\n"
    name = ARCHIVE.format("0.1.1")
    with tarfile.open(assets / name, "w:gz") as package:
        for filename, data in [("herdr-crossroads", variant), ("herdr-crossroads.sha256",
                               f"{digest(variant)}  herdr-crossroads\n".encode())]:
            entry = tarfile.TarInfo(filename)
            entry.size = len(data)
            entry.mode = 0o755 if filename == "herdr-crossroads" else 0o644
            package.addfile(entry, io.BytesIO(data))
    (assets / (name + ".sha256")).write_text(f"{digest((assets / name).read_bytes())}  {name}\n")
    return repo, assets, {"0.1.0": original, "0.1.1": variant}


def fixture_next_version(repo):
    for name in ["Cargo.toml", "herdr-plugin.toml"]:
        path = repo / name
        text, count = re.subn(r'^version = "0\.1\.0"$', 'version = "0.1.1"',
                              path.read_text(), flags=re.M)
        assert count == 1, name
        path.write_text(text)
    path = repo / "Cargo.lock"
    text, count = re.subn(r'(\[\[package\]\]\nname = "herdr-crossroads"\nversion = )"0\.1\.0"',
                          r'\1"0.1.1"', path.read_text())
    assert count == 1
    path.write_text(text)


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


def stop_owned(process):
    if process is None:
        return
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=3)


def run_case(herdr, asset_dir):
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="xrd-native-") as temporary:
        base = Path(temporary)
        repo, assets, payloads = prepare_fixture(base, asset_dir)
        tools = base / "bin"
        tools.mkdir()
        for name, executable in [("herdr", herdr), ("python3", Path(sys.executable)),
                                 ("git", shutil.which("git")), ("stty", shutil.which("stty")),
                                 ("sh", "/bin/sh")]:
            assert executable, f"required installed tool missing: {name}"
            (tools / name).symlink_to(Path(executable).resolve(strict=True))
        denied = base / "denied-tools"
        for name in ["cargo", "rustc", "rustup"]:
            path = tools / name
            path.write_text(f"#!{sys.executable}\nfrom pathlib import Path\n"
                            f"Path({str(denied)!r}).write_text({name!r})\nraise SystemExit(99)\n")
            path.chmod(0o755)
        env = {"PATH": str(tools), "LC_ALL": "C.UTF-8", "TERM": "xterm-256color",
               "HOME": str(base / "home"), "TMPDIR": str(base / "tmp"),
               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": str(base / "gitconfig"),
               "GIT_ALLOW_PROTOCOL": "file", "GIT_TERMINAL_PROMPT": "0",
               "CROSSROADS_ASSET_DIR": str(assets),
               "HERDR_CONFIG_PATH": str(base / "config/herdr.toml")}
        for kind in ["CONFIG", "STATE", "DATA", "CACHE", "RUNTIME"]:
            env[f"XDG_{kind}_HOME" if kind != "RUNTIME" else "XDG_RUNTIME_DIR"] = str(base / kind.lower())
        for key in ["HOME", "TMPDIR", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_DATA_HOME",
                    "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"]:
            Path(env[key]).mkdir(mode=0o700)
        Path(env["HERDR_CONFIG_PATH"]).write_text(
            "onboarding = false\n[terminal]\ndefault_shell = '/bin/sh'\nshell_mode = 'non_login'\n"
            "[update]\nversion_check = false\nmanifest_check = false\n")
        Path(env["GIT_CONFIG_GLOBAL"]).write_text(
            f'[url "{repo.as_uri()}"]\n    insteadOf = https://github.com/{SOURCE}.git\n'
            '[protocol]\n    allow = never\n[protocol "file"]\n    allow = always\n'
            '[core]\n    hooksPath = /dev/null\n'
            '[user]\n    name = Distribution Fixture\n    email = fixture@example.invalid\n')
        name = "distribution"
        session_dir = Path(env["XDG_CONFIG_HOME"]) / "herdr/sessions" / name
        api_path = session_dir / "herdr.sock"
        client_path = session_dir / "herdr-client.sock"
        assert api_path.resolve().is_relative_to(base) and client_path.resolve().is_relative_to(base)
        argv = [str(herdr), "--session", name]
        server = client = None
        master = slave = None

        def command(args, cwd=base):
            process = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                       text=True, start_new_session=True)
            try:
                stdout, stderr = process.communicate(timeout=30)
            except BaseException:
                # CLI hooks/Git descendants share this owned group. Do not leave
                # them writing into a fixture after its directory is removed.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.communicate(timeout=3)
                raise
            return subprocess.CompletedProcess(args, process.returncode, stdout, stderr)

        def cli(*args):
            result = command([*argv, *args])
            assert result.returncode == 0, (args, result.stdout, result.stderr)
            return result

        def rpc(method, params=None):
            assert api_path.resolve(strict=True).is_relative_to(base)
            with socket.socket(socket.AF_UNIX) as connection:
                connection.settimeout(2)
                connection.connect(str(api_path))
                connection.sendall(json.dumps({"id": "distribution", "method": method,
                                               "params": params or {}}).encode() + b"\n")
                with connection.makefile("rb") as stream:
                    reply = json.loads(stream.readline(1024 * 1024))
            assert reply["id"] == "distribution", reply
            return reply

        def result(method, params=None):
            reply = rpc(method, params)
            assert "error" not in reply, reply
            return reply["result"]

        def snapshot():
            return result("session.snapshot")["snapshot"]

        screen = Screen()

        def drain(duration=0.15):
            deadline = time.monotonic() + duration
            while time.monotonic() < deadline:
                if select.select([master], [], [], min(0.05, deadline - time.monotonic()))[0]:
                    data = os.read(master, 65536)
                    assert data, "native client PTY closed"
                    screen.feed(data)

        def wait_for(check, label, timeout=5):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if master is not None:
                    drain(0.05)
                if check():
                    if master is None:
                        return
                    drain(0.15)
                    if check():
                        return
                time.sleep(0.02)
            raise AssertionError(f"timed out: {label}\n{screen.text()}")

        def open_popup():
            cli("plugin", "pane", "open", "--plugin", "herdr-crossroads", "--entrypoint", "crossroads")

        def no_popup():
            # Exactly one mutation-capable probe. Ok would close a lingering OWN
            # popup and FAIL, never count as proof or be polled into a passing state.
            reply = rpc("popup.close")
            assert reply.get("error", {}).get("code") == "popup_not_open", reply

        def navigate(origin, arrival):
            result("pane.focus", {"pane_id": origin})
            wait_for(lambda: snapshot()["focused_pane_id"] == origin, "baseline focus")
            pane_ids = {p["pane_id"] for p in snapshot()["panes"]}
            open_popup()
            wait_for(lambda: "Origin" in screen.popup_view() and "Arrival" in screen.popup_view()
                     and "Select" in screen.popup_view(), "unfiltered native popup table")
            assert {p["pane_id"] for p in snapshot()["panes"]} == pane_ids, "popup entered ordinary pane topology"
            os.write(master, b"/Arrival")

            def filtered(mode):
                text = screen.popup_view()
                lines = [line for line in text.splitlines() if "Arrival" in line]
                return bool(re.search(r'Search: Arrival\s+' + mode + r'\b', text)) and len(lines) == 2 and "Origin" not in text

            wait_for(lambda: filtered("Search"), "fresh search query and sole Arrival row")
            assert snapshot()["focused_pane_id"] == origin
            os.write(master, b"\r")
            wait_for(lambda: filtered("Select"), "first Enter retains filter")
            assert snapshot()["focused_pane_id"] == origin
            duplicate = command([*argv, "plugin", "pane", "open", "--plugin", "herdr-crossroads",
                                 "--entrypoint", "crossroads"])
            assert duplicate.returncode != 0, duplicate
            busy = json.loads(duplicate.stderr)["error"]
            assert busy["code"] == "ui_busy" and "popup pane is already open" in busy["message"], busy
            os.write(master, b"\r")
            wait_for(lambda: snapshot()["focused_pane_id"] == arrival, "second Enter exact stable-ID focus")
            drain(0.5)
            no_popup()
            open_popup()  # Independent repeatability/closure proof; fresh empty query.
            wait_for(lambda: "Origin" in screen.popup_view() and "Arrival" in screen.popup_view()
                     and "Search: / to search" in screen.popup_view(), "reopened unfiltered native table")
            os.write(master, b"q")
            drain(0.5)
            no_popup()
            assert snapshot()["focused_pane_id"] == arrival

        with (base / "server.log").open("wb") as log:
            try:
                version = command([str(herdr), "--version"])
                assert version.returncode == 0 and re.fullmatch(r"herdr 0\.9\.3\s*", version.stdout), version
                commits = {}
                for args in [["init", "--quiet"], ["add", "."], ["commit", "--quiet", "-m", "Local distribution fixture"],
                             ["tag", "v0.1.0"]]:
                    outcome = command([str(tools / "git"), *args], cwd=repo)
                    assert outcome.returncode == 0, outcome.stderr
                commits["0.1.0"] = command([str(tools / "git"), "rev-parse", "HEAD"], cwd=repo).stdout.strip()
                fixture_next_version(repo)
                for args in [["add", "."], ["commit", "--quiet", "-m", "Test-only trailer fixture"], ["tag", "v0.1.1"]]:
                    outcome = command([str(tools / "git"), *args], cwd=repo)
                    assert outcome.returncode == 0, outcome.stderr
                commits["0.1.1"] = command([str(tools / "git"), "rev-parse", "HEAD"], cwd=repo).stdout.strip()
                source_before = command([str(tools / "git"), "status", "--porcelain"], cwd=repo).stdout
                artifact_before = {p.name: digest(p.read_bytes()) for p in assets.iterdir()}
                server = subprocess.Popen([*argv, "server"], cwd=base, env=env,
                                          stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
                wait_for(lambda: api_path.exists() and client_path.exists(), "owned server sockets")
                assert stat.S_ISSOCK(api_path.stat().st_mode) and stat.S_ISSOCK(client_path.stat().st_mode)
                assert result("ping")["version"] == "0.9.3"
                master, slave = pty.openpty()
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
                client = subprocess.Popen([*argv, "client"], cwd=base, env=env, stdin=slave,
                                          stdout=slave, stderr=slave, start_new_session=True,
                                          preexec_fn=lambda: fcntl.ioctl(0, termios.TIOCSCTTY, 0))
                os.close(slave)
                slave = None
                roots = []
                for label, pane_label in [("Source", "Origin"), ("Destination", "Arrival")]:
                    cwd = base / ("shell-" + label.lower())
                    cwd.mkdir()
                    created = json.loads(cli("workspace", "create", "--cwd", str(cwd), "--label", label, "--focus").stdout)["result"]
                    assert created["workspace"]["workspace_id"] and created["tab"]["tab_id"]
                    pane_id = created["root_pane"]["pane_id"]
                    roots.append(pane_id)
                    cli("pane", "rename", pane_id, pane_label)
                assert roots[0] != roots[1]
                assert len(snapshot()["panes"]) <= 3, "fixture table crop assumes at most three root shells"
                retained = []
                for version in ["0.1.0", "0.1.1", "0.1.0"]:
                    cli("plugin", "install", SOURCE, "--yes", "--ref", "v" + version)
                    plugins = result("plugin.list")["plugins"]
                    installed = [p for p in plugins if p["plugin_id"] == "herdr-crossroads"]
                    assert len(installed) == 1, plugins
                    plugin = installed[0]
                    assert plugin["version"] == version
                    source = plugin["source"]
                    assert source["owner"] == "SimonEisenhauer" and source["repo"] == "herdr-crossroads"
                    assert source["requested_ref"] == "v" + version and source["resolved_commit"] == commits[version]
                    managed = Path(source["managed_path"]).resolve(strict=True)
                    assert managed.is_relative_to(Path(env["XDG_CONFIG_HOME"])) and managed.is_relative_to(base)
                    binary = managed / "target/release/herdr-crossroads"
                    assert not binary.is_symlink() and binary.resolve(strict=True).is_relative_to(managed)
                    assert binary.read_bytes() == payloads[version] and binary.stat().st_mode & 0o777 == 0o755
                    assert not denied.exists(), "Rust/Cargo attempted"
                    if not retained:
                        # Discover only fixture-owned per-plugin roots; never invent
                        # user config paths or count managed source as retained config.
                        for kind in ["CONFIG", "STATE"]:
                            directories = [p for p in Path(env[f"XDG_{kind}_HOME"]).rglob("herdr-crossroads")
                                           if p.is_dir() and not p.resolve().is_relative_to(managed)
                                           and not managed.is_relative_to(p.resolve())]
                            assert len(directories) == 1, (kind, directories)
                            assert directories[0].resolve(strict=True).is_relative_to(base)
                            marker = directories[0] / "distribution-retained-marker"
                            marker.write_text("owned fixture data\n")
                            retained.append(marker)
                    navigate(*roots)
                    print(json.dumps({"version": version, "ref": source["requested_ref"],
                                      "commit": source["resolved_commit"], "binary_sha256": digest(binary.read_bytes()),
                                      "archive_sha256": digest((assets / ARCHIVE.format(version)).read_bytes()),
                                      "test_only_version": version == "0.1.1",
                                      "focused_pane_id": roots[1], "native_navigation": "passed"}), flush=True)
                cli("plugin", "uninstall", "herdr-crossroads")
                assert not any(p["plugin_id"] == "herdr-crossroads" for p in result("plugin.list")["plugins"])
                assert not managed.exists()
                assert all(p.read_text() == "owned fixture data\n" for p in retained)
                assert not denied.exists()
                assert artifact_before == {p.name: digest(p.read_bytes()) for p in assets.iterdir()}
                assert source_before == command([str(tools / "git"), "status", "--porcelain"], cwd=repo).stdout
                assert commits["0.1.1"] == command([str(tools / "git"), "rev-parse", "HEAD"], cwd=repo).stdout.strip()
            except BaseException:
                log.flush()
                print((base / "server.log").read_text(errors="replace")[-8000:], file=sys.stderr)
                raise
            finally:
                # All commands receive the clean environment. Never stop default
                # or inherited sessions. Fallback signals only owned Popen groups.
                try:
                    if server is not None and server.poll() is None:
                        stopped = command([str(herdr), "session", "stop", name])
                        assert stopped.returncode == 0, (stopped.stdout, stopped.stderr)
                finally:
                    try:
                        stop_owned(client)
                    finally:
                        try:
                            stop_owned(server)
                        finally:
                            for fd in [master, slave]:
                                if fd is not None:
                                    os.close(fd)
        for path in [api_path, client_path]:
            if path.exists():
                with socket.socket(socket.AF_UNIX) as connection:
                    connection.settimeout(0.5)
                    try:
                        connection.connect(str(path))
                    except OSError as error:
                        assert error.errno in [errno.ENOENT, errno.ECONNREFUSED], error
                    else:
                        raise AssertionError(f"owned socket still reachable: {path}")
        print("PASS native local installation, search/focus, replacement, version return, uninstall and shutdown")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--herdr-bin", type=Path, required=True)
    parser.add_argument("--asset-dir", type=Path, required=True)
    args = parser.parse_args()
    if not args.herdr_bin.is_absolute() or not args.asset_dir.is_absolute():
        parser.error("explicit absolute real Herdr binary and trusted assets required")
    herdr = args.herdr_bin.resolve(strict=True)
    assets = args.asset_dir.resolve(strict=True)
    with herdr.open("rb") as executable:
        if executable.read(4) != b"\x7fELF" or not os.access(herdr, os.X_OK):
            parser.error("Herdr must be a real executable ELF, not a PATH shim")
    if not assets.is_dir() or sys.flags.optimize:
        parser.error("asset directory required; assertions must remain enabled (no python -O)")
    run_case(herdr, assets)


if __name__ == "__main__":
    main()
