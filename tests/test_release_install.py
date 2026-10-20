"""Opt-in native release installation gate; never uses a personal Herdr session.

Run with an existing Herdr 0.9.5 executable and this host's packaged asset.
Git is redirected to a local fixture; release downloads are tested separately.
"""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
SOURCE = "eisenhauer-dev/herdr-crossroads"


def command(argv, *, cwd, env, timeout=180):
    process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except BaseException:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.communicate()
        raise
    result = subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)
    return result


def checked(argv, **kwargs):
    result = command(argv, **kwargs)
    if result.returncode:
        raise AssertionError(f"{argv}: {result.returncode}\n{result.stdout}\n{result.stderr}")
    return result


def checksum(data, name):
    return hashlib.sha256(data).hexdigest().encode() + b"  " + name.encode() + b"\n"


def pack(members):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        for name, data, kind in members:
            member = tarfile.TarInfo(name)
            member.type = kind
            member.mode = 0o755 if name == "herdr-crossroads" else 0o644
            member.size = len(data) if kind == tarfile.REGTYPE else 0
            member.linkname = "herdr-crossroads" if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE) else ""
            archive.addfile(member, io.BytesIO(data) if member.size else None)
    return output.getvalue()


def run(herdr, asset):
    herdr = herdr.resolve(strict=True)
    asset = asset.resolve(strict=True)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if platform.system() == "Darwin":
        target = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}[platform.machine()]
    elif platform.system() == "Linux" and platform.machine() == "x86_64":
        target = "x86_64-unknown-linux-gnu"
    else:
        raise ValueError("native release installation gate requires a supported host")
    name = f"herdr-crossroads-{version}-{target}.tar.gz"
    assert asset.name == name, (asset.name, name)
    archive_data = asset.read_bytes()
    assert asset.with_name(name + ".sha256").read_bytes() == checksum(archive_data, name)
    with tarfile.open(fileobj=io.BytesIO(archive_data), mode="r:gz") as package:
        members = package.getmembers()
        assert len(members) == 4
        assert {m.name for m in members} == {
            "herdr-crossroads", "herdr-crossroads.sha256", "LICENSE", "THIRD-PARTY-NOTICES.md"}
        assert all(m.isfile() and not m.issparse() for m in members)
        assert package.getmember("herdr-crossroads").mode & 0o111 == 0o111
        payloads = {m.name: package.extractfile(m).read() for m in members}
    assert payloads["herdr-crossroads.sha256"] == checksum(payloads["herdr-crossroads"], "herdr-crossroads")
    for notice in ("LICENSE", "THIRD-PARTY-NOTICES.md"):
        assert payloads[notice] == (ROOT / notice).read_bytes()

    # Canonical paths avoid macOS /tmp -> /private/tmp aliases without weakening
    # the installer's refusal to traverse symlink ancestors.
    with tempfile.TemporaryDirectory(dir="/tmp/opencode", prefix="crossroads-install-") as temporary:
        base = Path(temporary).resolve(strict=True)
        repo, assets, tools = (base / part for part in ("repo", "assets", "bin"))
        for directory in (repo, assets, tools):
            directory.mkdir()
        (repo / "scripts").mkdir()
        for relative in ("Cargo.toml", "Cargo.lock", "herdr-plugin.toml", "LICENSE",
                         "THIRD-PARTY-NOTICES.md", "scripts/check-version.py",
                         "scripts/install-binary.py", "scripts/setup-keybind.py"):
            shutil.copyfile(ROOT / relative, repo / relative)
        (assets / name).write_bytes(archive_data)
        (assets / (name + ".sha256")).write_bytes(checksum(archive_data, name))
        for tool, executable in (("herdr", herdr), ("python3", sys.executable),
                                 ("git", shutil.which("git")), ("stty", shutil.which("stty")),
                                 ("cat", shutil.which("cat")),
                                 ("sh", "/bin/sh")):
            assert executable, tool
            (tools / tool).symlink_to(Path(executable).resolve(strict=True))
        denied = base / "compiler-invoked"
        for tool in ("cargo", "rustc", "rustup"):
            path = tools / tool
            path.write_text(f"#!{sys.executable}\nfrom pathlib import Path\n"
                            f"Path({str(denied)!r}).write_text({tool!r})\nraise SystemExit(99)\n")
            path.chmod(0o755)
        env = {"PATH": str(tools), "HOME": str(base / "home"), "TMPDIR": str(base / "tmp"),
               "LC_ALL": "C", "TERM": "xterm-256color", "GIT_CONFIG_NOSYSTEM": "1",
               "GIT_CONFIG_GLOBAL": str(base / "gitconfig"), "GIT_ALLOW_PROTOCOL": "file",
               "GIT_TERMINAL_PROMPT": "0", "CROSSROADS_ASSET_DIR": str(assets),
               "HERDR_CONFIG_PATH": str(base / "config/herdr.toml")}
        for kind in ("CONFIG", "STATE", "DATA", "CACHE", "RUNTIME"):
            env[f"XDG_{kind}_HOME" if kind != "RUNTIME" else "XDG_RUNTIME_DIR"] = str(base / kind.lower())
        for key in ("HOME", "TMPDIR", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_DATA_HOME",
                    "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"):
            Path(env[key]).mkdir(mode=0o700)
        config = Path(env["HERDR_CONFIG_PATH"])
        config.write_text("# preserved host configuration\nonboarding = false\n")
        config_before = config.read_bytes()
        Path(env["GIT_CONFIG_GLOBAL"]).write_text(
            f'[url "{repo.as_uri()}"]\n    insteadOf = https://github.com/{SOURCE}.git\n'
            '[protocol]\n    allow = never\n[protocol "file"]\n    allow = always\n'
            '[core]\n    hooksPath = /dev/null\n[user]\n'
            '    name = Distribution Fixture\n    email = fixture@example.invalid\n')
        kwargs = {"cwd": base, "env": env}
        assert checked([str(herdr), "--version"], **kwargs).stdout.strip() == "herdr 0.9.5"
        for args in (["init", "--quiet"], ["add", "."], ["commit", "--quiet", "-m", "Release installation fixture"]):
            checked([str(tools / "git"), *args], cwd=repo, env=env)
        commit = checked([str(tools / "git"), "rev-parse", "HEAD"], cwd=repo, env=env).stdout.strip()
        # Discover the canonical plugin configuration path using the real CLI.
        plugin_config = Path(checked([str(herdr), "plugin", "config-dir", "herdr-crossroads"], **kwargs).stdout.strip())
        assert plugin_config.resolve().is_relative_to(base)
        retained = plugin_config / "installation-retained.txt"
        retained.write_text("preserve user plugin configuration\n")
        installed = None
        for attempt in range(2):
            checked([str(herdr), "plugin", "install", SOURCE, "--yes", "--ref", commit], **kwargs)
            listing = json.loads(checked([str(herdr), "plugin", "list", "--json"], **kwargs).stdout)
            plugins = listing["result"]["plugins"]
            assert len(plugins) == 1, plugins
            plugin = plugins[0]
            assert plugin["plugin_id"] == "herdr-crossroads" and plugin["version"] == version
            assert plugin["source"]["resolved_commit"] == commit
            assert plugin["source"]["requested_ref"] == commit
            managed = Path(plugin["source"]["managed_path"]).resolve(strict=True)
            assert managed.is_relative_to(base)
            manifest = tomllib.loads((managed / "herdr-plugin.toml").read_text())
            pane = next(p for p in manifest["panes"] if p["id"] == "crossroads")
            assert pane["command"] == ["./target/release/herdr-crossroads"]
            installed = managed / pane["command"][0]
            assert installed.read_bytes() == payloads["herdr-crossroads"]
            assert installed.stat().st_mode & 0o777 == 0o755
            assert not installed.is_symlink()
            for notice in ("LICENSE", "THIRD-PARTY-NOTICES.md"):
                assert (installed.parent / notice).read_bytes() == payloads[notice]
            assert config.read_bytes() == config_before
            assert retained.read_text() == "preserve user plugin configuration\n"
            assert not denied.exists(), "binary installation invoked Rust tooling"
            if attempt == 0:
                for gate in ("latency", "render", "keymaps", "hints"):
                    result = checked([sys.executable, "-u", str(ROOT / f"tests/pty_{gate}.py"), str(installed)],
                                     cwd=ROOT, env=env, timeout=300)
                    print(result.stdout, end="", flush=True)

        # Exercise native host and binary validation, not a mocked platform, while
        # retaining a known existing executable through every rejected package.
        binary = payloads["herdr-crossroads"]
        installer = installed.parents[2] / "scripts/install-binary.py"
        checked([sys.executable, str(installer)], **kwargs)
        wrong_arch = bytearray(binary)
        if target.endswith("darwin"):
            wrong_arch[4:8] = (0x01000007 if "aarch64" in target else 0x0100000C).to_bytes(4, "little")
        else:
            wrong_arch[18:20] = (183).to_bytes(2, "little")
        original = [(key, value, tarfile.REGTYPE) for key, value in payloads.items()]
        cases = {}
        for label, replacement in (("wrong architecture", bytes(wrong_arch)), ("wrong format", b"not an executable")):
            changed = {**payloads, "herdr-crossroads": replacement,
                       "herdr-crossroads.sha256": checksum(replacement, "herdr-crossroads")}
            cases[label] = [(key, value, tarfile.REGTYPE) for key, value in changed.items()]
        cases["inner checksum"] = [(key, b"0" * 64 + b"  herdr-crossroads\n" if key == "herdr-crossroads.sha256" else value, kind)
                                   for key, value, kind in original]
        cases["missing member"] = original[:-1]
        cases["extra member"] = original + [("extra", b"unexpected", tarfile.REGTYPE)]
        for unsafe in ("../herdr-crossroads", "/herdr-crossroads"):
            cases[unsafe] = [(unsafe if key == "herdr-crossroads" else key, value, kind) for key, value, kind in original]
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE):
            cases[f"link {kind!r}"] = [(key, value, kind if key == "herdr-crossroads" else member_kind)
                                      for key, value, member_kind in original]
        for notice in ("LICENSE", "THIRD-PARTY-NOTICES.md"):
            cases[f"tampered {notice}"] = [(key, value + b"tampered" if key == notice else value, kind)
                                          for key, value, kind in original]
        for label, entries in cases.items():
            rejected = pack(entries)
            (assets / name).write_bytes(rejected)
            (assets / (name + ".sha256")).write_bytes(checksum(rejected, name))
            outcome = command([sys.executable, str(installer)], **kwargs)
            assert outcome.returncode != 0, label
            assert "Binary installation failed:" in outcome.stderr, (label, outcome.stderr)
            assert installed.read_bytes() == binary, f"existing installation changed: {label}"
        (assets / name).write_bytes(archive_data)
        (assets / (name + ".sha256")).write_bytes(b"0" * 64 + b"  " + name.encode() + b"\n")
        assert command([sys.executable, str(installer)], **kwargs).returncode != 0
        assert installed.read_bytes() == binary
        (assets / name).unlink()
        assert command([sys.executable, str(installer)], **kwargs).returncode != 0
        assert installed.read_bytes() == binary
        (assets / name).write_bytes(archive_data)
        (assets / (name + ".sha256")).write_bytes(checksum(archive_data, name))
        failed_curl = tools / "curl"
        failed_curl.write_text("#!/bin/sh\nexit 22\n")
        failed_curl.chmod(0o755)
        online = dict(env)
        online.pop("CROSSROADS_ASSET_DIR")
        rejected_download = command([sys.executable, str(installer)], cwd=base, env=online)
        assert rejected_download.returncode != 0
        assert "curl release download failed (exit status 22)" in rejected_download.stderr
        assert installed.read_bytes() == binary
        release = installed.parent
        preserved = release.with_name("preserved-release")
        release.rename(preserved)
        release.symlink_to(preserved, target_is_directory=True)
        try:
            assert command([sys.executable, str(installer)], **kwargs).returncode != 0
            assert installed.read_bytes() == binary
        finally:
            release.unlink()
            preserved.rename(release)
        assert config.read_bytes() == config_before and not denied.exists()
        checked([str(herdr), "plugin", "uninstall", "herdr-crossroads"], **kwargs)
        assert config.read_bytes() == config_before
        assert retained.read_text() == "preserve user plugin configuration\n"
        # The optional setup action is tested only against this owned config;
        # installation itself must never add a host binding.
        foreign = b'\n[[keys.command]]\nkey = "prefix+x"\ncommand = "echo retained"\n'
        config.write_bytes(config_before + foreign)
        checked([sys.executable, str(repo / "scripts/setup-keybind.py")], **kwargs)
        configured = config.read_bytes()
        assert configured.startswith(config_before + foreign)
        checked([sys.executable, str(repo / "scripts/setup-keybind.py")], **kwargs)
        assert config.read_bytes() == configured
        print(f"Native Herdr 0.9.5 install/reinstall, all four installed PTYs and rejection checks passed: {target}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("herdr", type=Path)
    parser.add_argument("archive", type=Path)
    args = parser.parse_args()
    run(args.herdr, args.archive)
