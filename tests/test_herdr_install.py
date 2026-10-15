"""MAIN-OWNED opt-in real Herdr 0.9.3/0.9.5 CLI test; never run by test discovery.

Protocol/fixture contract: herdrdev/herdr v0.9.3 and v0.9.5 tests/cli/harness.rs,
tests/cli/plugins.rs, src/cli/plugin.rs, src/protocol/wire.rs;
v0.9.5 src/persist/plugin_registry.rs and src/api/schema/plugins.rs.
"""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import subprocess
import sys
import tarfile
import tempfile
import threading

from test_distribution import SOURCE, digest, enable_public_downloads, install_public, public_version


ROOT = Path(__file__).resolve().parents[1]


def run_case(herdr, asset_dir, archive_name, case, public_ref=None, public_commit=None):
    version_number = public_version(public_ref, public_commit) if public_ref else "0.1.0"
    requested_ref = public_commit if public_ref else "v0.1.0"
    owner, repository = SOURCE.split("/") if public_ref else ("fixture", "crossroads")
    source_slug = owner + "/" + repository
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-cli-") as temporary:
        base = Path(temporary)
        repo = base / "source"
        (repo / "scripts").mkdir(parents=True)
        for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml", "LICENSE", "THIRD-PARTY-NOTICES.html"]:
            shutil.copy2(ROOT / name, repo / name)
        for name in ["check-version.py", "install-binary.py"]:
            shutil.copy2(ROOT / "scripts" / name, repo / "scripts" / name)
        assets = base / "assets"
        assets.mkdir()
        for name in [archive_name, archive_name + ".sha256"]:
            shutil.copy2(asset_dir / name, assets / name)
        archive = assets / archive_name
        if public_ref:
            assert (assets / (archive_name + ".sha256")).read_text() == (
                f"{digest(archive.read_bytes())}  {archive_name}\n")
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
                        f"{hashlib.sha256(wrong).hexdigest()}  herdr-crossroads\n".encode()),
                        ("LICENSE", (repo / "LICENSE").read_bytes()),
                        ("THIRD-PARTY-NOTICES.html", (repo / "THIRD-PARTY-NOTICES.html").read_bytes())]:
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
        if public_ref:
            enable_public_downloads(env)

        def command(argv, cwd=base):
            process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                       text=True, start_new_session=True)
            try:
                stdout, stderr = process.communicate(timeout=180 if public_ref else 30)
            except BaseException:
                # Kill only this command's group, including its Git/build descendants.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.communicate(timeout=3)
                raise
            return subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)

        version = command([str(herdr), "--version"])
        assert version.returncode == 0 and re.fullmatch(r"herdr 0\.9\.(?:3|5)\s*", version.stdout), version
        runtime_version = version.stdout.strip().split()[1]
        if public_ref:
            assert runtime_version == "0.9.5", "public acceptance requires Herdr 0.9.5"
        for args in ([] if public_ref else [["init", "--quiet"], ["add", "."],
                     ["-c", "user.name=Installer Fixture", "-c", "user.email=fixture@example.invalid",
                      "-c", "core.hooksPath=/dev/null", "commit", "--quiet", "-m", "Installer fixture"],
                     ["tag", "v0.1.0"]]):
            result = command([git, *args], cwd=repo)
            assert result.returncode == 0, result.stderr
        commit = public_commit if public_ref else command([git, "rev-parse", "HEAD"], cwd=repo).stdout.strip()
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
                                response = {"type": "pong", "version": runtime_version, "protocol": 22,
                                            "capabilities": {"live_handoff": True, "detached_server_daemon": True}}
                            elif method == "plugin.list":
                                response = {"type": "plugin_list", "plugins": []}
                            elif method == "plugin.link":
                                assert runtime_version == "0.9.3", "0.9.5 install must persist its registry directly"
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
                                for name in ["LICENSE", "THIRD-PARTY-NOTICES.html"]:
                                    notice = binary.parent / name
                                    assert not notice.is_symlink()
                                    assert notice.read_bytes() == (repo / name).read_bytes()
                                    assert notice.stat().st_mode & 0o777 == 0o644
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
            if public_ref:
                result = install_public(command, [str(herdr)], public_ref, public_commit)
            else:
                result = command([str(herdr), "plugin", "install", source_slug,
                                  "--yes", "--ref", requested_ref])
        finally:
            stop.set()
            server.join(timeout=6)
            listener.close()
        assert not server.is_alive(), "fake API did not stop"
        assert not failures, failures
        assert "plugin.list" in requests, "installed-plugin lookup must use the sandbox API"
        assert not denied.exists(), "installer attempted Rust/Cargo"
        registries = list(Path(env["XDG_CONFIG_HOME"]).rglob("plugins.json"))
        if runtime_version == "0.9.3" or case != "success":
            assert not registries, "registry must not be written by 0.9.3 or a failed build"
        assert not list(Path(env["XDG_CONFIG_HOME"]).rglob(".tmp-install-*")), "temporary checkout leaked"
        if case == "success":
            assert result.returncode == 0, (result.stdout, result.stderr)
            if runtime_version == "0.9.3":
                assert len(linked) == 1 and requests.count("plugin.link") == 1
            else:
                assert not linked and "plugin.link" not in requests
                assert registries == [Path(env["XDG_CONFIG_HOME"]) / "herdr/plugins.json"], registries
                registry = registries[0]
                assert not registry.is_symlink() and registry.resolve(strict=True).is_relative_to(base)
                plugins = json.loads(registry.read_text())
                assert isinstance(plugins, list) and len(plugins) == 1, plugins
                plugin = plugins[0]
                assert plugin["plugin_id"] == "herdr-crossroads" and plugin["version"] == version_number
                assert plugin["name"] == "Herdr Crossroads" and plugin["min_herdr_version"] == "0.9.1"
                assert plugin["enabled"] is True and not plugin.get("warnings")
                source = plugin["source"]
                assert source["kind"] == "github" and source.get("subdir") is None
                assert source["owner"] == owner and source["repo"] == repository
                assert source["requested_ref"] == requested_ref and source["resolved_commit"] == commit
                assert type(source["installed_unix_ms"]) is int and source["installed_unix_ms"] > 0
                managed = Path(source["managed_path"]).resolve(strict=True)
                assert managed.is_relative_to(Path(env["XDG_CONFIG_HOME"])) and managed.is_relative_to(base)
                assert Path(plugin["plugin_root"]).resolve(strict=True) == managed
                assert Path(plugin["manifest_path"]).resolve(strict=True) == managed / "herdr-plugin.toml"
                for name, data, mode in [("herdr-crossroads", expected, 0o755),
                                         ("LICENSE", (repo / "LICENSE").read_bytes(), 0o644),
                                         ("THIRD-PARTY-NOTICES.html", (repo / "THIRD-PARTY-NOTICES.html").read_bytes(), 0o644)]:
                    installed = managed / "target/release" / name
                    assert not installed.is_symlink() and installed.resolve(strict=True).is_relative_to(managed)
                    assert installed.read_bytes() == data and installed.stat().st_mode & 0o777 == mode
                assert f"Installed herdr-crossroads from {source_slug}." in result.stdout
        else:
            assert result.returncode != 0 and "plugin build failed" in result.stderr, result.stderr
            assert "plugin.link" not in requests and not linked
            assert not list(Path(env["XDG_CONFIG_HOME"]).rglob("target/release/herdr-crossroads"))
        print(f"PASS real {version.stdout.strip()} {'public download' if public_ref else 'local'} install: {case}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--herdr-bin", type=Path, required=True)
    parser.add_argument("--asset-dir", type=Path, required=True)
    parser.add_argument("--public-ref", help="OPT IN to public GitHub checkout and Release downloads (vX.Y.Z)")
    parser.add_argument("--public-commit", help="reviewed full commit SHA required with --public-ref")
    args = parser.parse_args()
    if args.public_ref is not None or args.public_commit is not None:
        try:
            public_version(args.public_ref, args.public_commit)
        except ValueError as error:
            parser.error(str(error))
    if sys.flags.optimize:
        parser.error("assertions must remain enabled (no python -O)")
    if not args.herdr_bin.is_absolute() or not args.asset_dir.is_absolute():
        parser.error("explicit absolute real Herdr executable and trusted asset directory required")
    herdr = args.herdr_bin.resolve(strict=True)
    with herdr.open("rb") as executable:
        if executable.read(4) != b"\x7fELF" or not os.access(herdr, os.X_OK):
            parser.error("Herdr must be a real executable ELF, not a PATH shim")
    version = args.public_ref[1:] if args.public_ref else "0.1.0"
    for case in (["success"] if args.public_ref else ["success", "missing-asset", "missing-checksum", "bad-hash", "wrong-target"]):
        run_case(herdr, args.asset_dir, f"herdr-crossroads-{version}-x86_64-unknown-linux-gnu.tar.gz",
                 case, args.public_ref, args.public_commit)


if __name__ == "__main__":
    main()
