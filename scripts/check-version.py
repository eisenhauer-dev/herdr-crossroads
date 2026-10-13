"""Print the agreed stable release version; never write build/workflow metadata."""

from pathlib import Path
import re
import sys
import tomllib


def release_version():
    root = Path(__file__).resolve().parents[1]
    data = {}
    for name in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
        with (root / name).open("rb") as source:
            data[name] = tomllib.load(source)
    package = data["Cargo.toml"]["package"]
    if not isinstance(package, dict) or package.get("name") != "herdr-crossroads":
        raise ValueError("Cargo.toml must describe herdr-crossroads")
    packages = data["Cargo.lock"]["package"]
    if not isinstance(packages, list) or not all(isinstance(entry, dict) for entry in packages):
        raise ValueError("Cargo.lock package entries must be tables")
    entries = [entry for entry in packages if entry.get("name") == "herdr-crossroads"]
    if len(entries) != 1:
        raise ValueError("Cargo.lock must contain exactly one herdr-crossroads package")
    versions = [package["version"], entries[0]["version"], data["herdr-plugin.toml"]["version"]]
    for version in versions:
        if not isinstance(version, str) or not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
            raise ValueError("only canonical stable X.Y.Z release versions are supported")
    if len(set(versions)) != 1:
        raise ValueError("Cargo.toml, Cargo.lock, and herdr-plugin.toml versions must match")
    return versions[0]


if __name__ == "__main__":
    try:
        print(release_version())
    except (OSError, UnicodeError, ValueError, KeyError) as error:
        print(f"Release version check failed: {error}", file=sys.stderr)
        sys.exit(1)
