"""Generate cached exact-version notices, or cheaply reject stale build inputs."""

import hashlib
import html
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"
NOTICE = ROOT / "THIRD-PARTY-NOTICES.html"


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def fingerprint():
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    manifest["package"].pop("version")
    for package in lock["package"]:
        if package["name"] == "herdr-crossroads" and "source" not in package:
            package.pop("version")
    data = json.dumps([manifest, lock], sort_keys=True).encode()
    return hashlib.sha256(data).hexdigest()


def system_notices():
    output = ["<h1>System startup and runtime provenance</h1>",
              "<p>Installed native GCC driver candidates, resolved paths, file hashes "
              "and Debian/Ubuntu package ownership follow. This is current-builder "
              "provenance, not a historical linker map or proof that every archive "
              "member was linked. Shared libraries are not bundled. Full package-wide "
              "copyright records include unrelated architectures/languages; their "
              "inclusion does not make all their licenses apply to Crossroads.</p>",
              "<p>GCC's installed record explicitly covers libgcc and crtstuff.c "
              "under GPL-3.0-or-later with GCC Runtime Library Exception 3.1. "
              "The exact glibc source headers below additionally establish "
              "unlimited linking/distribution exceptions for eight named startup "
              "and nonshared sources. Those exceptions do not extend to other "
              "glibc sources or versions. Final matching ELF/link-map review must "
              "verify which sources actually contribute to the binary.</p>"]
    owners = set()
    for name in ["Scrt1.o", "crti.o", "crtn.o", "crtbeginS.o", "crtendS.o",
                 "libc.so", "libc_nonshared.a", "libgcc.a", "libgcc_eh.a", "libgcc_s.so",
                 "libc.so.6", "libgcc_s.so.1", "ld-linux-x86-64.so.2"]:
        path = Path(command("gcc", f"-print-file-name={name}")).resolve(strict=True)
        ownership = command("dpkg-query", "-S", str(path))
        if len(ownership.splitlines()) != 1 or ": " not in ownership:
            raise ValueError(f"ambiguous package ownership: {path}")
        owner = ownership.split(": ", 1)[0]
        owners.add(owner)
        if name == "Scrt1.o":
            source = command("dpkg-query", "-W", "-f=${source:Package} ${source:Version}", owner)
            evidence = ROOT / "LICENSES/glibc-2.43-2ubuntu2.4-startup.txt"
            text = evidence.read_text()
            if text.splitlines()[0] != "Source: " + source:
                raise ValueError("glibc startup source evidence does not match installed source package")
            output += ["<h2>Verified glibc startup source exceptions</h2>",
                       f"<p>{evidence.relative_to(ROOT)}; "
                       f"SHA256: {hashlib.sha256(evidence.read_bytes()).hexdigest()}</p>",
                       "<pre>" + html.escape(text) + "</pre>"]
        output += [f"<h2>{name}</h2>",
                   f"<p>{html.escape(str(path))}; owner: {html.escape(owner)}; "
                   f"SHA256: {hashlib.sha256(path.read_bytes()).hexdigest()}</p>"]
    notices = set()
    for owner in sorted(owners):
        record = command("dpkg-query", "-W", "-f=${binary:Package} ${Version}; "
                         "source: ${source:Package} ${source:Version}", owner)
        output.append(f"<p>{html.escape(record)}</p>")
        notices.add((Path("/usr/share/doc") / owner.split(":", 1)[0] / "copyright").resolve(strict=True))
    licenses = set()
    for path in sorted(notices):
        text = path.read_text()
        output += [f"<h2>{html.escape(str(path))}</h2>",
                   "<pre>" + html.escape(text) + "</pre>"]
        for name in re.findall(r"/usr/share/common-licenses/([A-Za-z0-9.-]+)", text):
            licenses.add((Path("/usr/share/common-licenses") / name.rstrip(".")).resolve(strict=True))
    for path in sorted(licenses):
        output += [f"<h2>{html.escape(str(path))}</h2>",
                   "<pre>" + html.escape(path.read_text()) + "</pre>"]
    for path in sorted(notices | licenses):
        owner = command("dpkg-query", "-S", str(path)).split(": ", 1)[0]
        record = command("dpkg-query", "-W", "-f=${binary:Package} ${Version}", owner)
        output.append(f"<p>{html.escape(str(path))}; provider: {html.escape(record)}; "
                      f"Notice/license file SHA256: {hashlib.sha256(path.read_bytes()).hexdigest()}</p>")
    return output


def generate():
    tree = command("cargo", "tree", "--offline", "--locked", "--target", TARGET,
                   "-e", "normal,build", "--prefix", "none")
    active = set(re.findall(r"^([\w-]+) v([^\s]+)", tree, re.M))
    metadata = json.loads(command("cargo", "metadata", "--offline", "--locked",
                                  "--format-version", "1", "--filter-platform", TARGET))
    packages = sorted((p for p in metadata["packages"]
                       if p["source"] and (p["name"], p["version"]) in active),
                      key=lambda p: (p["name"], p["version"]))
    checksums = {(p["name"], p["version"]): p.get("checksum", "")
                 for p in tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]}
    if len(packages) != len(active) - 1:
        raise ValueError("active package inventory does not match metadata")
    toolchain = command("rustc", "--version")
    sysroot = Path(command("rustc", "--print", "sysroot"))
    standard_library = (sysroot / "share/doc/rust/COPYRIGHT-library.html").read_text()
    verbose = command("rustc", "--version", "--verbose")
    commit = re.search(r"^commit-hash: ([0-9a-f]{40})$", verbose, re.M)
    if commit is None:
        raise ValueError("Rust commit provenance unavailable")
    body = standard_library.split("<body>", 1)[1].rsplit("</body>", 1)[0]
    output = ["<!DOCTYPE html>\n<html><head><meta charset=\"UTF-8\">"
              "<title>Crossroads third-party notices</title></head><body>",
              f"<!-- inputs-sha256: {fingerprint()} -->",
              f"<!-- rustc: {html.escape(toolchain)} -->",
              "<h1>Crossroads third-party notices</h1>",
              f"<p>Target: {TARGET}. Active normal + build graph: {len(packages)} "
              "unique registry package versions. This is a conservative inventory, "
              "including build-only and proc-macro dependencies, not a claim that "
              "all packages are linked into the executable. Dev-only and inactive "
              "target dependencies are excluded.</p>",
              "<p>Source: exact Cargo.lock versions, cached crates.io package manifests "
              "and all packaged license, copyright, NOTICE and attribution files. "
              "Texts below are reproduced without modification (HTML-escaped). "
              "License expressions describe upstream terms, not license selections. "
              "Project MIT terms are delivered separately in LICENSE.</p>",
              "<p>Regenerate with python3 scripts/third-party-notices.py --write; "
              "review dependency and toolchain changes before distribution.</p>"]
    for package in packages:
        name, version = package["name"], package["version"]
        output += [f"<h2>{name} {version}</h2>",
                   f"<p>Source: https://crates.io/crates/{name}/{version}; "
                   f"Cargo.lock checksum: {checksums[name, version]}</p>",
                   f"<p>License expression: {html.escape(package['license'] or '')}</p>"]
        root = Path(package["manifest_path"]).parent
        files = sorted(p for p in root.rglob("*") if p.is_file() and
                       re.search(r"licen[cs]e|notice|copyright|copying|attribution", p.name, re.I))
        if not files:
            raise ValueError(f"no notice files found for {name} {version}")
        for path in files:
            output += [f"<h3>{html.escape(str(path.relative_to(root)))}</h3>",
                       "<pre>" + html.escape(path.read_text()) + "</pre>"]
    output += ["<h1>Rust standard library (conservative upstream inventory)</h1>",
               f"<p>Rust source commit: https://github.com/rust-lang/rust/tree/{commit[1]}. "
               f"Upstream COPYRIGHT-library.html SHA256: {hashlib.sha256(standard_library.encode()).hexdigest()}.</p>",
               f"<p>{html.escape(toolchain)}. Source: installed toolchain "
               "share/doc/rust/COPYRIGHT-library.html, reproduced below in full. "
               "This includes upstream's cross-target/build inventory; inclusion "
               "does not imply every component is linked on Linux.</p>", body]
    output += system_notices()
    output.append("</body></html>\n")
    return "\n".join(output)


def main():
    if NOTICE.is_symlink():
        raise ValueError("notice output must not be a symlink")
    if sys.argv[1:] == ["--write"]:
        NOTICE.write_text(generate())
    elif sys.argv[1:] == ["--check"]:
        if not (ROOT / "LICENSE").is_file() or (ROOT / "LICENSE").is_symlink():
            raise ValueError("release licenses must be regular non-symlink files")
        text = NOTICE.read_text()
        if f"<!-- inputs-sha256: {fingerprint()} -->" not in text:
            raise ValueError("stale third-party notices: review and regenerate")
        if f"<!-- rustc: {html.escape(command('rustc', '--version'))} -->" not in text:
            raise ValueError("Rust toolchain changed: review and regenerate notices")
    else:
        raise ValueError("usage: third-party-notices.py --check | --write")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, IndexError, subprocess.CalledProcessError) as error:
        sys.exit(f"Third-party notice validation failed: {error}")
