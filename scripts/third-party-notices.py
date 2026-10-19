"""Generate exact-version Markdown notices, or cheaply reject stale build inputs."""

import hashlib
import html
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"
NOTICE = ROOT / "THIRD-PARTY-NOTICES.md"


class Markdown(HTMLParser):
    """Convert the reviewed notice HTML vocabulary; fail on upstream changes.

    Preformatted text is decoded once by HTMLParser and never stripped, wrapped,
    or escaped. A longer fence prevents license text from closing its own block.
    """

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.blocks = []
        self.text = ""
        self.pre = None
        self.head = False
        self.depth = 0
        self.prefix = ""
        self.links = []
        self.anchors = {}
        self.heading_id = None

    def flush(self):
        text = self.text.strip()
        if text:
            self.blocks.append("> " * self.depth + self.prefix + text)
        self.text = ""
        self.prefix = ""

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        allowed = {"html", "head", "meta", "title", "body", "h1", "h2", "h3",
                   "p", "pre", "b", "code", "a", "ul", "li", "div", "details", "summary"}
        permitted = {"meta": {"charset"}, "div": {"style"}, "a": {"href"},
                     "h1": {"id"}, "h2": {"id"}, "h3": {"id"}}
        if tag not in allowed or set(attributes) - permitted.get(tag, set()):
            raise ValueError(f"unsupported notice HTML: {tag} {attrs}")
        if self.pre is not None:
            raise ValueError("unsupported markup inside preformatted notice")
        if tag == "head":
            self.head = True
        if self.head:
            return
        if tag in {"h1", "h2", "h3", "p", "pre", "li", "div", "details", "summary", "ul"}:
            self.flush()
        if tag in {"h1", "h2", "h3"}:
            self.prefix = "#" * int(tag[1]) + " "
            self.heading_id = attributes.get("id")
        elif tag == "div":
            self.depth += 1
        elif tag == "li":
            self.prefix = "- "
        elif tag == "pre":
            self.pre = ""
        elif tag == "b":
            self.text += "**"
        elif tag == "a":
            self.links.append(attributes["href"])
            self.text += "["

    def handle_endtag(self, tag):
        if tag == "head":
            self.head = False
            return
        if self.head:
            return
        if tag == "pre":
            fence = "`" * max(3, max((len(s) + 1 for s in re.findall(r"`+", self.pre)), default=0))
            self.blocks.append(fence + "\n" + self.pre + "\n" + fence)
            self.pre = None
        elif tag == "a":
            self.text += "](<" + self.links.pop() + ">)"
        elif tag == "b":
            self.text += "**"
        elif tag in {"h1", "h2", "h3", "p", "li", "div", "details", "summary", "ul"}:
            if tag in {"h1", "h2", "h3"} and self.heading_id:
                self.anchors[self.heading_id] = re.sub(r"[^\w -]", "", self.text.lower()).replace(" ", "-")
                self.heading_id = None
            self.flush()
            if tag == "div":
                self.depth -= 1

    def handle_data(self, data):
        if self.head:
            return
        if self.pre is not None:
            self.pre += data
        else:
            # HTML prose whitespace is collapsible, unlike preformatted text.
            self.text += re.sub(r"([\\`*_{}\[\]<>])", r"\\\1", re.sub(r"\s+", " ", data))

    def handle_comment(self, data):
        if not self.head:
            self.flush()
            self.blocks.append("<!--" + data + "-->")


def markdown(source):
    parser = Markdown()
    parser.feed(source)
    parser.close()
    parser.flush()
    if parser.pre is not None:
        raise ValueError("unterminated preformatted notice")
    # Keep the document title first, before machine-readable provenance comments.
    title = next((i for i, block in enumerate(parser.blocks) if block.startswith("# ")), None)
    if title is not None:
        parser.blocks.insert(0, parser.blocks.pop(title))
    result = "\n\n".join(parser.blocks) + "\n"
    for anchor, slug in parser.anchors.items():
        result = result.replace("](<#" + anchor + ">)", "](<#" + slug + ">)")
    return result


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


def installed_system():
    candidates = []
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
        version, source, source_version = command(
            "dpkg-query", "-W", "-f=${Version} ${source:Package} ${source:Version}", owner).split()
        candidates.append(dict(query=name, resolved=str(path),
                               sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                               packages=[dict(name=owner, version=version, source=source,
                                              source_version=source_version)]))
    notices = set()
    for owner in sorted(owners):
        notices.add((Path("/usr/share/doc") / owner.split(":", 1)[0] / "copyright").resolve(strict=True))
    licenses = set()
    for path in sorted(notices):
        text = path.read_text()
        for name in re.findall(r"/usr/share/common-licenses/([A-Za-z0-9.-]+)", text):
            licenses.add((Path("/usr/share/common-licenses") / name.rstrip(".")).resolve(strict=True))
    return candidates, [dict(path=str(path), text=path.read_text(),
                             sha256=hashlib.sha256(path.read_bytes()).hexdigest())
                        for path in sorted(notices | licenses)]


def system_notices(validate_installed=True):
    output = ["<h1>System startup and runtime provenance</h1>",
              "<p>Conservative inventory of two reviewed builders, in stable order. "
              "Only one profile must match the current builder's installed candidates "
              "and full notice/license bytes. Neither profile proves actual code "
              "contributions: reconcile the matching raw link map and final ELF before "
              "distribution. GCC candidates may differ from actual driver flags. "
              "Shared libraries are not bundled. Package-wide records include unrelated "
              "architectures/languages; inclusion does not make all their licenses apply.</p>",
              "<p>GCC records cover libgcc and crtstuff.c under GPL-3.0-or-later with "
              "GCC Runtime Library Exception 3.1, for eligible compilation processes. "
              "The glibc exceptions cover only the eight named sources at each exact "
              "source version, not other glibc sources or arbitrary GPL code.</p>"]
    installed = installed_system() if validate_installed else None
    matched = False
    for name, source in [("local-glibc-2.43", "2.43-2ubuntu2.4"),
                         ("ubuntu-24.04", "2.39-0ubuntu8.9")]:
        path = ROOT / f"LICENSES/{name}-system.json"
        data = path.read_bytes()
        profile = json.loads(data)
        evidence = ROOT / f"LICENSES/glibc-{source}-startup.txt"
        raw = evidence.read_bytes()
        expected = dict(path=str(evidence.relative_to(ROOT)),
                        sha256=hashlib.sha256(raw).hexdigest(), source="glibc " + source)
        if (profile["glibc_evidence"] != expected
                or raw.decode().splitlines()[0] != "Source: glibc " + source):
            raise ValueError("glibc startup source evidence does not match reviewed profile")
        for notice in profile["notices"]:
            if hashlib.sha256(notice["text"].encode()).hexdigest() != notice["sha256"]:
                raise ValueError("reviewed system notice hash mismatch")
        matched |= installed == (profile["candidates"], profile["notices"])
        output += [f"<h2>{html.escape(profile['label'])}</h2>",
                   f"<p>{path.relative_to(ROOT)}; SHA256: {hashlib.sha256(data).hexdigest()}</p>",
                   "<pre>" + html.escape(json.dumps(profile["provenance"], indent=2)) + "</pre>"]
        for candidate in profile["candidates"]:
            output += [f"<h3>{html.escape(candidate['query'])}</h3>",
                       "<pre>" + html.escape(json.dumps(candidate, indent=2)) + "</pre>"]
        output += ["<h3>Verified glibc startup source exceptions</h3>",
                   f"<p>{evidence.relative_to(ROOT)}; SHA256: {expected['sha256']}</p>",
                   "<pre>" + html.escape(raw.decode()) + "</pre>"]
        for notice in profile["notices"]:
            output += [f"<h3>{html.escape(notice['path'])}</h3>",
                       f"<p>Notice/license file SHA256: {notice['sha256']}</p>",
                       "<pre>" + html.escape(notice["text"]) + "</pre>"]
    if validate_installed and not matched:
        raise ValueError("installed inputs do not match either reviewed system builder; review required")
    return output


def generate_html():
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
    standard_library = (sysroot / "share/doc/rust/COPYRIGHT-library.html").read_bytes().decode()
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
                       "<pre>" + html.escape(path.read_bytes().decode()) + "</pre>"]
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


def generate():
    return markdown(generate_html()).replace(
        "Texts below are reproduced without modification (HTML-escaped).",
        "Texts below are reproduced without modification in verbatim fenced blocks.")


def check_darwin(target):
    """Darwin's graph is a subset; verify attribution, never invoke gcc/dpkg there.

    Native system static inputs are separately rejected using the actual Apple
    link map in build-provenance.py. Only OS-provided dylibs may remain external.
    """
    if target not in {"aarch64-apple-darwin", "x86_64-apple-darwin"}:
        raise ValueError("unsupported Darwin notice target")
    text = NOTICE.read_bytes().decode()
    if f"<!-- inputs-sha256: {fingerprint()} -->" not in text:
        raise ValueError("stale third-party notices: review and regenerate")
    version = command("rustc", "--version")
    if not version.startswith("rustc 1.99.0 ") or f"<!-- rustc: {html.escape(version)} -->" not in text:
        raise ValueError("Rust toolchain changed: review and regenerate notices")
    tree = command("cargo", "tree", "--offline", "--locked", "--target", target,
                   "-e", "normal,build", "--prefix", "none")
    active = set(re.findall(r"^([\w-]+) v([^\s]+)", tree, re.M))
    metadata = json.loads(command("cargo", "metadata", "--offline", "--locked",
                                  "--format-version", "1", "--filter-platform", target))
    packages = [p for p in metadata["packages"] if p["source"] and (p["name"], p["version"]) in active]
    if len(packages) != len(active) - 1:
        raise ValueError("active Darwin package inventory does not match metadata")
    for package in packages:
        heading = markdown(f"<h2>{html.escape(package['name'])} {package['version']}</h2>").strip()
        if "\n" + heading + "\n" not in text:
            raise ValueError("Darwin dependency missing from reviewed notices")
        files = sorted(p for p in Path(package["manifest_path"]).parent.rglob("*") if p.is_file() and
                       re.search(r"licen[cs]e|notice|copyright|copying|attribution", p.name, re.I))
        if not files or any("\n" + p.read_bytes().decode() + "\n" not in text for p in files):
            raise ValueError("Darwin dependency attribution missing or changed")
    sysroot = Path(command("rustc", "--print", "sysroot"))
    standard = (sysroot / "share/doc/rust/COPYRIGHT-library.html").read_bytes()
    if f"Upstream COPYRIGHT-library.html SHA256: {hashlib.sha256(standard).hexdigest()}." not in text:
        raise ValueError("Rust standard library notice changed: review required")
    body = standard.decode().split("<body>", 1)[1].rsplit("</body>", 1)[0]
    if markdown(body).strip() not in text:
        raise ValueError("Rust standard library attribution missing")
    if not text.endswith(markdown("\n".join(system_notices(validate_installed=False)))):
        raise ValueError("reviewed Linux system attribution missing or changed")


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
        if not text.endswith(markdown("\n".join(system_notices()))):
            raise ValueError("stale system notices: review and regenerate")
    elif len(sys.argv) == 4 and sys.argv[1:3] == ["--check", "--target"]:
        if not (ROOT / "LICENSE").is_file() or (ROOT / "LICENSE").is_symlink():
            raise ValueError("release licenses must be regular non-symlink files")
        check_darwin(sys.argv[3])
    else:
        raise ValueError("usage: third-party-notices.py --check | --write")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, IndexError, subprocess.CalledProcessError) as error:
        sys.exit(f"Third-party notice validation failed: {error}")
