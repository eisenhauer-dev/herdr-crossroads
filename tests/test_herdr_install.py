"""MAIN-OWNED opt-in real Herdr 0.9.3 CLI test; never run by test discovery.

Protocol/fixture contract: herdrdev/herdr v0.9.3 tests/cli/harness.rs,
tests/cli/plugins.rs, src/cli/plugin.rs, src/protocol/wire.rs.
"""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import threading


ROOT = Path(__file__).resolve().parents[1]


def run_case(herdr, asset_dir, archive_name, case):
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-cli-") as temporary:
        base = Path(temporary)
        repo = base / "source"
        (repo / "scripts").mkdir(parents=True)
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
            shutil.copy2(ROOT / name, repo / name)
        for name in ["check-version.py", "install-binary.py"]:
            shutil.copy2(ROOT / "scripts" / name, repo / "scripts" / name)
        assets = base / "assets"
        assets.mkdir()
        for name in [archive_name, archive_name + ".sha256"]:
            shutil.copy2(asset_dir / name, assets / name)
        archive = assets / archive_name
        with tarfile.open(archive) as package:
            expected = package.extractfile("herdr-crossroads").read()
        if case == "missing-asset":
            archive.unlink()
        elif case == "missing-checksum":
            (assets / (archive_name + ".sha256")).unlink()
        elif case == "bad-hash":
            archive.write_bytes(b"corrupted archive")
        elif case == "wrong-target":
            wrong = expected[:18] + b"\xb7\x00" + expected[20:]
            with tarfile.open(archive, "w:gz") as package:
                for name, data in [("herdr-crossroads", wrong), ("herdr-crossroads.sha256",
                        f"{hashlib.sha256(wrong).hexdigest()}  herdr-crossroads\n".encode())]:
                    entry = tarfile.TarInfo(name)
                    entry.size = len(data)
                    package.addfile(entry, io.BytesIO(data))
            (assets / (archive_name + ".sha256")).write_text(
                f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive_name}\n")

        tools = base / "bin"
        tools.mkdir()
        git = shutil.which("git")
        if git is None:
            raise RuntimeError("real Git is required")
        (tools / "git").symlink_to(Path(git).resolve())
        (tools / "python3").symlink_to(Path(sys.executable).resolve())
        denied = base / "denied-tools"
        for name in ["cargo", "rustc", "rustup"]:
            tool = tools / name
            tool.write_text(f"#!{sys.executable}\nfrom pathlib import Path\n"
                            f"Path({str(denied)!r}).write_text({name!r})\nraise SystemExit(99)\n")
            tool.chmod(0o755)
        # Nothing inherited: no runtime/session/plugin, credentials, SSH or Git env.
        env = {"PATH": str(tools), "LC_ALL": "C", "HOME": str(base / "home"),
               "TMPDIR": str(base / "tmp"), "GIT_CONFIG_NOSYSTEM": "1",
               "GIT_CONFIG_GLOBAL": str(base / "gitconfig"), "GIT_ALLOW_PROTOCOL": "file",
               "GIT_TERMINAL_PROMPT": "0", "CROSSROADS_ASSET_DIR": str(assets),
               "HERDR_CONFIG_PATH": str(base / "config/herdr.toml"),
               "HERDR_SOCKET_PATH": str(base / "api.sock")}
        for kind in ["CONFIG", "STATE", "DATA", "CACHE", "RUNTIME"]:
            env[f"XDG_{kind}_HOME" if kind != "RUNTIME" else "XDG_RUNTIME_DIR"] = str(base / kind.lower())
        for key in ["HOME", "TMPDIR", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_DATA_HOME",
                    "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"]:
            Path(env[key]).mkdir(mode=0o700)
        Path(env["HERDR_CONFIG_PATH"]).write_text("")
        (base / "gitconfig").write_text(
            f'[url "{repo.as_uri()}"]\n    insteadOf = https://github.com/fixture/crossroads.git\n'
            '[protocol]\n    allow = never\n[protocol "file"]\n    allow = always\n')

        def command(argv, cwd=base):
            return subprocess.run(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                  text=True, capture_output=True, timeout=30)

        version = command([str(herdr), "--version"])
        assert version.returncode == 0 and re.fullmatch(r"herdr 0\.9\.3\s*", version.stdout), version
        for args in [["init", "--quiet"], ["add", "."],
                     ["-c", "user.name=Installer Fixture", "-c", "user.email=fixture@example.invalid",
                      "-c", "core.hooksPath=/dev/null", "commit", "--quiet", "-m", "Installer fixture"],
                     ["tag", "v0.1.0"]]:
            result = command([git, *args], cwd=repo)
            assert result.returncode == 0, result.stderr
        commit = command([git, "rev-parse", "HEAD"], cwd=repo).stdout.strip()
        requests = []
        linked = []
        failures = []
        stop = threading.Event()
        listener = socket.socket(socket.AF_UNIX)
        listener.bind(env["HERDR_SOCKET_PATH"])
        listener.listen()
        listener.settimeout(0.1)

        def serve():
            try:
                while not stop.is_set():
                    try:
                        connection, _ = listener.accept()
                    except socket.timeout:
                        continue
                    with connection:
                        connection.settimeout(5)
                        with connection.makefile("rwb") as stream:
                            request = json.loads(stream.readline())
                            method = request["method"]
                            requests.append(method)
                            if method == "ping":
                                response = {"type": "pong", "version": "0.9.3", "protocol": 22,
                                            "capabilities": {"live_handoff": True, "detached_server_daemon": True}}
                            elif method == "plugin.list":
                                response = {"type": "plugin_list", "plugins": []}
                            elif method == "plugin.link":
                                assert case == "success", "failed build must not register"
                                params = request["params"]
                                source = params["source"]
                                managed = Path(source["managed_path"]).resolve()
                                assert managed.is_relative_to(Path(env["XDG_CONFIG_HOME"]))
                                assert source["requested_ref"] == "v0.1.0"
                                assert source["resolved_commit"] == commit
                                assert source["owner"] == "fixture" and source["repo"] == "crossroads"
                                binary = managed / "target/release/herdr-crossroads"
                                assert binary.read_bytes() == expected
                                assert binary.stat().st_mode & 0o777 == 0o755
                                linked.append(managed)
                                response = {"type": "plugin_linked", "plugin": {
                                    "plugin_id": "herdr-crossroads", "name": "Herdr Crossroads",
                                    "version": "0.1.0", "min_herdr_version": "0.9.1",
                                    "manifest_path": params["path"], "plugin_root": str(managed),
                                    "enabled": params["enabled"], "source": source}}
                            else:
                                raise AssertionError(f"unexpected API operation: {method}")
                            stream.write(json.dumps({"id": request["id"], "result": response}).encode() + b"\n")
                            stream.flush()
            except BaseException as error:
                failures.append(error)

        server = threading.Thread(target=serve)
        server.start()
        try:
            short = command([str(herdr), "plugin", "install", "herdr-crossroads", "--yes"])
            assert short.returncode == 2 and "usage:" in short.stderr, short
            assert requests == [], "single slug must fail before API access"
            result = command([str(herdr), "plugin", "install", "fixture/crossroads",
                              "--yes", "--ref", "v0.1.0"])
        finally:
            stop.set()
            server.join(timeout=6)
            listener.close()
        assert not server.is_alive(), "fake API did not stop"
        assert not failures, failures
        assert "plugin.list" in requests, "sandbox API must be used, not offline registration"
        assert not denied.exists(), "installer attempted Rust/Cargo"
        assert not list(Path(env["XDG_CONFIG_HOME"]).rglob("plugins.json")), "no offline registry permitted"
        assert not list(Path(env["XDG_CONFIG_HOME"]).rglob(".tmp-install-*")), "temporary checkout leaked"
        if case == "success":
            assert result.returncode == 0 and len(linked) == 1, (result.stdout, result.stderr)
            assert requests.count("plugin.link") == 1
        else:
            assert result.returncode != 0 and "plugin build failed" in result.stderr, result.stderr
            assert "plugin.link" not in requests and not linked
            assert not list(Path(env["XDG_CONFIG_HOME"]).rglob("target/release/herdr-crossroads"))
        print(f"PASS real Herdr 0.9.3 local install: {case}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--herdr-bin", type=Path, required=True)
    parser.add_argument("--asset-dir", type=Path, required=True)
    args = parser.parse_args()
    if not args.herdr_bin.is_absolute() or not args.asset_dir.is_absolute():
        parser.error("explicit absolute real Herdr executable and trusted asset directory required")
    herdr = args.herdr_bin.resolve(strict=True)
    with herdr.open("rb") as executable:
        if executable.read(4) != b"\x7fELF" or not os.access(herdr, os.X_OK):
            parser.error("Herdr must be a real executable ELF, not a PATH shim")
    for case in ["success", "missing-asset", "missing-checksum", "bad-hash", "wrong-target"]:
        run_case(herdr, args.asset_dir, "herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz", case)


if __name__ == "__main__":
    main()
