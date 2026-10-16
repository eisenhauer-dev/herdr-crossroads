"""Explicitly append the Crossroads host binding; never launch or reload Herdr."""

import os
from pathlib import Path
import secrets
import stat
import sys
import tomllib


COMMAND = "herdr plugin pane open --plugin herdr-crossroads --entrypoint crossroads"
BINDING = f'''\n\n[[keys.command]]
key = "prefix+f"
type = "shell"
command = "{COMMAND}"
description = "Open Herdr Crossroads"
'''.encode()


def config_path():
    for variable, suffix in [("HERDR_CONFIG_PATH", ""),
                             ("XDG_CONFIG_HOME", "herdr/config.toml"),
                             ("HOME", ".config/herdr/config.toml")]:
        if variable in os.environ:
            value = os.environ[variable]
            path = Path(value)
            if not value or not path.is_absolute() or ".." in path.parts:
                raise ValueError(f"{variable} must be nonempty, absolute, and without '..'; no fallback")
            if not suffix and (not path.name or value.endswith("/")):
                raise ValueError("HERDR_CONFIG_PATH must name a file, not a directory path")
            return path / suffix if suffix else path
    raise ValueError("HOME is required when HERDR_CONFIG_PATH and XDG_CONFIG_HOME are absent")


def binding_values(value):
    if isinstance(value, str):
        return [value]
    if isinstance(value, list) and all(isinstance(item, str) for item in value):
        return value
    raise ValueError("keys bindings must be strings or arrays of strings")


def has_launch_binding(data):
    keys = tomllib.loads(data.decode("utf-8")).get("keys", {})
    if not isinstance(keys, dict):
        raise ValueError("keys must be a table")
    conflict = "prefix+f is occupied or reserved; keep existing bindings and choose a free key manually"
    for name, value in keys.items():
        if name == "command":
            continue
        if name == "indexed":
            # Legacy modifier-only shortcuts expand to direct number keys 1-9, never prefix+f.
            if not isinstance(value, dict):
                raise ValueError("keys.indexed must be a table")
            for target, modifiers in value.items():
                if target not in ("tabs", "workspaces", "agents") or not isinstance(modifiers, str):
                    raise ValueError("keys.indexed supports only tabs, workspaces and agents strings")
                if modifiers.strip() and any(
                        token.strip().lower() not in ("ctrl", "control", "shift", "alt", "option",
                                                      "meta", "cmd", "command", "super", "hyper")
                        for token in modifiers.split("+")):
                    raise ValueError(f"keys.indexed.{target} must be empty or a modifier combination")
            continue
        values = binding_values(value)
        if name in ("prefix", "extra_prefixes"):
            if any(raw.strip() == "f" for raw in values):
                raise ValueError(conflict + f" (keys.{name})")
        elif any(is_launch_key(raw) for raw in values):
            raise ValueError(conflict + f" (keys.{name})")
    commands = keys.get("command", [])
    if not isinstance(commands, list):
        raise ValueError("keys.command must be an array of tables")
    found = False
    for entry in commands:
        if (not isinstance(entry, dict) or "key" not in entry
                or not isinstance(entry.get("command"), str)
                or entry.get("type", "shell") not in ("shell", "pane", "popup", "plugin_action")):
            raise ValueError("malformed keys.command entry; review config manually")
        values = binding_values(entry["key"])
        if any(is_launch_key(raw) for raw in values):
            if entry.get("type", "shell") != "shell" or entry["command"] != COMMAND:
                raise ValueError(conflict + " (keys.command)")
            found = True
    return found


def is_launch_key(raw):
    # Herdr trims both the binding and body; prefix+ is case-sensitive.
    # F and modified f (e.g. ctrl+f) are distinct from unmodified f.
    raw = raw.strip()
    return raw.startswith("prefix+") and raw[len("prefix+"):].strip() == "f"


def resolved_config_path(path):
    """Resolve existing links strictly, allowing only genuinely missing components."""
    missing = []
    while True:
        try:
            path.lstat()
            break
        except FileNotFoundError:
            missing.append(path.name)
            path = path.parent
    try:
        return path.resolve(strict=True).joinpath(*reversed(missing))
    except RuntimeError as error:  # Python 3.11/3.12 report symlink loops this way.
        raise ValueError(f"cannot resolve config path: {error}") from error


def parent_directory(path):
    """Create missing parents, refusing symlink ancestors, anchored by descriptors."""
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    directory = os.open("/", flags)
    try:
        for part in path.parent.parts[1:]:
            try:
                os.mkdir(part, mode=0o700, dir_fd=directory)
            except FileExistsError:
                pass
            child = os.open(part, flags, dir_fd=directory)
            os.close(directory)
            directory = child
        return directory
    except BaseException:
        os.close(directory)
        raise


def file_identity(info):
    # Reads may update atime; compare only identity, permissions and edit metadata.
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def snapshot(directory, name):
    try:
        info = os.stat(name, dir_fd=directory, follow_symlinks=False)
    except FileNotFoundError:
        return None, b""
    if not stat.S_ISREG(info.st_mode):
        raise ValueError("config target must be a regular file, not a symlink or other file type")
    descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
    with os.fdopen(descriptor, "rb") as source:
        opened = os.fstat(source.fileno())
        if file_identity(opened) != file_identity(info):
            raise ValueError("config changed while reading; stop concurrent edits and retry")
        data = source.read()
        if file_identity(os.fstat(source.fileno())) != file_identity(opened):
            raise ValueError("config changed while reading; stop concurrent edits and retry")
    return opened, data


def setup():
    selected = config_path()
    path = resolved_config_path(selected)
    reported = str(selected) if selected == path else f"{selected} (resolved target: {path})"
    directory = parent_directory(path)
    staging = None
    try:
        original, data = snapshot(directory, path.name)
        if has_launch_binding(data):
            print(f"Already configured prefix+f in {reported}; no changes. Reload explicitly if needed.")
            return
        proposed = data + BINDING
        has_launch_binding(proposed)  # Validate append without rewriting existing TOML/comments.
        candidate = ".crossroads-keybind-" + secrets.token_hex(12)
        descriptor = os.open(candidate, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                             0o600, dir_fd=directory)
        staging = candidate
        with os.fdopen(descriptor, "wb") as output:
            output.write(proposed)
            output.flush()
            os.fchmod(output.fileno(), stat.S_IMODE(original.st_mode) if original else 0o600)
            os.fsync(output.fileno())
        current, current_data = snapshot(directory, path.name)
        if ((file_identity(current) if current else None)
                != (file_identity(original) if original else None) or current_data != data):
            raise ValueError("config changed before write; stop concurrent edits and retry")
        try:
            if resolved_config_path(selected) != path:
                raise ValueError("config target redirected")
            try:
                reachable = selected.stat()
            except FileNotFoundError:
                reachable = None
            if ((file_identity(reachable) if reachable else None)
                    != (file_identity(current) if current else None)):
                raise ValueError("config target identity changed")
        except (OSError, ValueError) as error:
            raise ValueError("config changed before write; stop concurrent edits and retry") from error
        os.replace(staging, path.name, src_dir_fd=directory, dst_dir_fd=directory)
        staging = None
    finally:
        if staging is not None:
            os.unlink(staging, dir_fd=directory)
        os.close(directory)
    print(f"Added prefix+f to {reported}. Now run: herdr server reload-config")


if __name__ == "__main__":
    try:
        if len(sys.argv) != 1:
            raise ValueError("setup-keybind takes no arguments")
        setup()
    except (OSError, ValueError, UnicodeError) as error:
        print(f"Keybinding setup failed: {error}", file=sys.stderr)
        sys.exit(1)
