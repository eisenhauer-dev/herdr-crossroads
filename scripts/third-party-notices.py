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

# Verbatim upstream v13.0.0 COPYING.MinGW-w64-runtime/COPYING.MinGW-w64-runtime.txt.
# Keep even upstream's unresolved component notes; this is not link clearance.
MINGW_RUNTIME = '''MinGW-w64 runtime licensing
***************************

This program or library was built using MinGW-w64 and statically
linked against the MinGW-w64 runtime. Some parts of the runtime
are under licenses which require that the copyright and license
notices are included when distributing the code in binary form.
These notices are listed below.


========================
Overall copyright notice
========================

Copyright (c) 2009, 2010, 2011, 2012, 2013 by the mingw-w64 project

This license has been certified as open source. It has also been designated
as GPL compatible by the Free Software Foundation (FSF).

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

   1. Redistributions in source code must retain the accompanying copyright
      notice, this list of conditions, and the following disclaimer.
   2. Redistributions in binary form must reproduce the accompanying
      copyright notice, this list of conditions, and the following disclaimer
      in the documentation and/or other materials provided with the
      distribution.
   3. Names of the copyright holders must not be used to endorse or promote
      products derived from this software without prior written permission
      from the copyright holders.
   4. The right to distribute this software or to use it for any purpose does
      not give you the right to use Servicemarks (sm) or Trademarks (tm) of
      the copyright holders.  Use of them is covered by separate agreement
      with the copyright holders.
   5. If any files are modified, you must cause the modified files to carry
      prominent notices stating that you changed the files and the date of
      any change.

Disclaimer

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS ``AS IS'' AND ANY EXPRESSED
OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO
EVENT SHALL THE COPYRIGHT HOLDERS BE LIABLE FOR ANY DIRECT, INDIRECT,
INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA,
OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE,
EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

======================================== 
getopt, getopt_long, and getop_long_only
======================================== 

Copyright (c) 2002 Todd C. Miller <Todd.Miller@courtesan.com> 
 
Permission to use, copy, modify, and distribute this software for any 
purpose with or without fee is hereby granted, provided that the above 
copyright notice and this permission notice appear in all copies. 
 	 
THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

Sponsored in part by the Defense Advanced Research Projects
Agency (DARPA) and Air Force Research Laboratory, Air Force
Materiel Command, USAF, under agreement number F39502-99-1-0512.

        *       *       *       *       *       *       * 

Copyright (c) 2000 The NetBSD Foundation, Inc.
All rights reserved.

This code is derived from software contributed to The NetBSD Foundation
by Dieter Baron and Thomas Klausner.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:
 1. Redistributions of source code must retain the above copyright
    notice, this list of conditions and the following disclaimer.
 2. Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in the
    documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE NETBSD FOUNDATION, INC. AND CONTRIBUTORS
``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE FOUNDATION OR CONTRIBUTORS
BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.


===============================================================
gdtoa: Converting between IEEE floating point numbers and ASCII
===============================================================

The author of this software is David M. Gay.

Copyright (C) 1997, 1998, 1999, 2000, 2001 by Lucent Technologies
All Rights Reserved

Permission to use, copy, modify, and distribute this software and
its documentation for any purpose and without fee is hereby
granted, provided that the above copyright notice appear in all
copies and that both that the copyright notice and this
permission notice and warranty disclaimer appear in supporting
documentation, and that the name of Lucent or any of its entities
not be used in advertising or publicity pertaining to
distribution of the software without specific, written prior
permission.

LUCENT DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE,
INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS.
IN NO EVENT SHALL LUCENT OR ANY OF ITS ENTITIES BE LIABLE FOR ANY
SPECIAL, INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER
IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.

        *       *       *       *       *       *       *

The author of this software is David M. Gay.

Copyright (C) 2005 by David M. Gay
All Rights Reserved

Permission to use, copy, modify, and distribute this software and its
documentation for any purpose and without fee is hereby granted,
provided that the above copyright notice appear in all copies and that
both that the copyright notice and this permission notice and warranty
disclaimer appear in supporting documentation, and that the name of
the author or any of his current or former employers not be used in
advertising or publicity pertaining to distribution of the software
without specific, written prior permission.

THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE,
INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS.  IN
NO EVENT SHALL THE AUTHOR OR ANY OF HIS CURRENT OR FORMER EMPLOYERS BE
LIABLE FOR ANY SPECIAL, INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY
DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS
SOFTWARE.

        *       *       *       *       *       *       *

The author of this software is David M. Gay.

Copyright (C) 2004 by David M. Gay.
All Rights Reserved
Based on material in the rest of /netlib/fp/gdota.tar.gz,
which is copyright (C) 1998, 2000 by Lucent Technologies.

Permission to use, copy, modify, and distribute this software and
its documentation for any purpose and without fee is hereby
granted, provided that the above copyright notice appear in all
copies and that both that the copyright notice and this
permission notice and warranty disclaimer appear in supporting
documentation, and that the name of Lucent or any of its entities
not be used in advertising or publicity pertaining to
distribution of the software without specific, written prior
permission.

LUCENT DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE,
INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS.
IN NO EVENT SHALL LUCENT OR ANY OF ITS ENTITIES BE LIABLE FOR ANY
SPECIAL, INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER
IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.


=========================
Parts of the math library
=========================

Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.

Developed at SunSoft, a Sun Microsystems, Inc. business.
Permission to use, copy, modify, and distribute this
software is freely granted, provided that this notice
is preserved.

        *       *       *       *       *       *       *

Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.

Developed at SunPro, a Sun Microsystems, Inc. business.
Permission to use, copy, modify, and distribute this
software is freely granted, provided that this notice
is preserved.

        *       *       *       *       *       *       *

FIXME: Cephes math lib
Copyright (C) 1984-1998 Stephen L. Moshier

It sounds vague, but as to be found at
<http://lists.debian.org/debian-legal/2004/12/msg00295.html>, it gives an
impression that the author could be willing to give an explicit
permission to distribute those files e.g. under a BSD style license. So
probably there is no problem here, although it could be good to get a
permission from the author and then add a license into the Cephes files
in MinGW runtime. At least on follow-up it is marked that debian sees the
version a-like BSD one. As MinGW.org (where those cephes parts are coming
from) distributes them now over 6 years, it should be fine.

===================================
Headers and IDLs imported from Wine
===================================

Some header and IDL files were imported from the Wine project. These files
are prominent maked in source. Their copyright belongs to contributors and
they are distributed under LGPL license.

Disclaimer

This library is free software; you can redistribute it and/or
modify it under the terms of the GNU Lesser General Public
License as published by the Free Software Foundation; either
version 2.1 of the License, or (at your option) any later version.

This library is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
Lesser General Public License for more details.
'''

WINPTHREADS = '''Copyright (c) 2011 mingw-w64 project

Permission is hereby granted, free of charge, to any person obtaining a
copy of this software and associated documentation files (the "Software"),
to deal in the Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.


/*
 * Parts of this library are derived by:
 *
 * Posix Threads library for Microsoft Windows
 *
 * Use at own risk, there is no implied warranty to this code.
 * It uses undocumented features of Microsoft Windows that can change
 * at any time in the future.
 *
 * (C) 2010 Lockless Inc.
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 *
 *  * Redistributions of source code must retain the above copyright notice,
 *    this list of conditions and the following disclaimer.
 *  * Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions and the following disclaimer in the documentation
 *    and/or other materials provided with the distribution.
 *  * Neither the name of Lockless Inc. nor the names of its contributors may be
 *    used to endorse or promote products derived from this software without
 *    specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AN
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
 * WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
 * LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE
 * OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
 * OF THE POSSIBILITY OF SUCH DAMAGE.
 */
'''


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
        expected = dict(path=evidence.relative_to(ROOT).as_posix(),
                        sha256=hashlib.sha256(raw).hexdigest(), source="glibc " + source)
        if (profile["glibc_evidence"] != expected
                or raw.decode().splitlines()[0] != "Source: glibc " + source):
            raise ValueError("glibc startup source evidence does not match reviewed profile")
        for notice in profile["notices"]:
            if hashlib.sha256(notice["text"].encode()).hexdigest() != notice["sha256"]:
                raise ValueError("reviewed system notice hash mismatch")
        matched |= installed == (profile["candidates"], profile["notices"])
        output += [f"<h2>{html.escape(profile['label'])}</h2>",
                   f"<p>{path.relative_to(ROOT).as_posix()}; SHA256: {hashlib.sha256(data).hexdigest()}</p>",
                   "<pre>" + html.escape(json.dumps(profile["provenance"], indent=2)) + "</pre>"]
        for candidate in profile["candidates"]:
            output += [f"<h3>{html.escape(candidate['query'])}</h3>",
                       "<pre>" + html.escape(json.dumps(candidate, indent=2)) + "</pre>"]
        output += ["<h3>Verified glibc startup source exceptions</h3>",
                   f"<p>{evidence.relative_to(ROOT).as_posix()}; SHA256: {expected['sha256']}</p>",
                   "<pre>" + html.escape(raw.decode()) + "</pre>"]
        for notice in profile["notices"]:
            output += [f"<h3>{html.escape(notice['path'])}</h3>",
                       f"<p>Notice/license file SHA256: {notice['sha256']}</p>",
                       "<pre>" + html.escape(notice["text"]) + "</pre>"]
    if validate_installed and not matched:
        raise ValueError("installed inputs do not match either reviewed system builder; review required")
    return output


def notice_files(package):
    root = Path(package["manifest_path"]).parent
    files = sorted(p for p in root.rglob("*") if p.is_file() and
                   re.search(r"licen[cs]e|notice|copyright|copying|attribution", p.name, re.I))
    if (package["name"], package["version"]) == ("winapi-x86_64-pc-windows-gnu", "0.4.0"):
        # This published crate omitted its license texts and contains an unrelated
        # binary libwinapi_oemlicense.a. Use the exact upstream workspace licenses,
        # also packaged by winapi 0.3.9; never decode an import archive as a notice.
        root = root.parent / "winapi-0.3.9"
        files = [root / name for name in ("LICENSE-APACHE", "LICENSE-MIT")]
        expected = ["b40930bbcf80744c86c46a12bc9da056641d722716c378f5659b9e555ef833e1",
                    "ce7bc3499fee93d5022ef430d5e4201e79a6d9154f3974e42f41349f0569e09b"]
        if [hashlib.sha256(p.read_bytes()).hexdigest() for p in files] != expected:
            raise ValueError("winapi GNU upstream workspace license mismatch")
    if not files:
        raise ValueError(f"no notice files found for {package['name']} {package['version']}")
    return root, files


def windows_notices():
    if hashlib.sha256(MINGW_RUNTIME.encode()).hexdigest() != "e9b2dc02451ea29092a1f25fa0f3c07207ed421f1807dffb0c4e6dce69dee7bd":
        raise ValueError("MinGW-w64 upstream runtime notice changed")
    return ["<h1>Windows GNU startup and runtime notices</h1>",
            "<p>Preparation target x86_64-pc-windows-gnu: MinGW-w64 13.0.0 POSIX/SEH/UCRT, "
            "GCC 15.2.0 and Binutils 2.46. No compiler or runtime DLL is bundled. "
            "This is conservative upstream attribution, not proof of the runner archive identity "
            "or final symbol contributions. Actual input hashes and member evidence require native GNU preparation. "
            "Review applicable component notices against that evidence before distribution.</p>",
            "<p>Source: https://github.com/mingw-w64/mingw-w64/blob/v13.0.0/"
            "COPYING.MinGW-w64-runtime/COPYING.MinGW-w64-runtime.txt; SHA256: "
            "e9b2dc02451ea29092a1f25fa0f3c07207ed421f1807dffb0c4e6dce69dee7bd. "
            "The full upstream notice follows, including its unresolved component notes. "
            "COPYING at the same tag specifies ZPL 2.1 except prominently marked public-domain, BSD or LGPL parts. "
            "Inclusion does not establish incorporation of every listed component.</p>",
            "<pre>" + html.escape(MINGW_RUNTIME) + "</pre>",
            "<p>Winpthreads source: https://github.com/mingw-w64/mingw-w64/blob/v13.0.0/"
            "mingw-w64-libraries/winpthreads/COPYING; SHA256: " + hashlib.sha256(WINPTHREADS.encode()).hexdigest() + ".</p>",
            "<pre>" + html.escape(WINPTHREADS) + "</pre>",
            "<p>GCC runtime: Copyright (C) 1989-2025 Free Software Foundation, Inc. "
            "Source: https://github.com/gcc-mirror/gcc/blob/releases/gcc-15.2.0/libgcc/libgcc2.c. "
            "GPL-3.0-or-later with GCC Runtime Library Exception 3.1. "
            "The full GPL version 3 and verbatim Exception 3.1 are reproduced in the system notices below. "
            "The exception permits eligible target code combined with independent modules under their own terms; "
            "it does not relicense GCC itself as MIT. The compiler and Binutils are build tools, not shipped components.</p>"]


def generate_html(windows=False):
    active = set()
    inventory = {}
    targets = [TARGET, "x86_64-pc-windows-gnu"] if windows else [TARGET]
    for target in targets:
        tree = command("cargo", "tree", "--offline", "--locked", "--target", target,
                       "-e", "normal,build", "--prefix", "none")
        selected = set(re.findall(r"^([\w-]+) v([^\s]+)", tree, re.M))
        active.update(selected)
        metadata = json.loads(command("cargo", "metadata", "--offline", "--locked",
                                      "--format-version", "1", "--filter-platform", target))
        inventory.update({(p["name"], p["version"]): p for p in metadata["packages"]
                          if p["source"] and (p["name"], p["version"]) in selected})
    packages = [inventory[key] for key in sorted(inventory)]
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
              (f"<p>Targets: {', '.join(targets)}. Union of active normal + build graphs: {len(packages)} "
               if windows else f"<p>Target: {TARGET}. Active normal + build graph: {len(packages)} ") +
              "unique registry package versions. This is a conservative inventory, "
              "including build-only and proc-macro dependencies, not a claim that "
              "all packages are linked into the executable. Dev-only and inactive " +
              ("target dependencies outside this union are excluded.</p>" if windows
               else "target dependencies are excluded.</p>"),
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
        if name == "winapi-x86_64-pc-windows-gnu":
            output += ["<p>License texts omitted from the published 0.4.0 crate: exact upstream "
                       "workspace licenses from https://github.com/retep998/winapi-rs/tree/0.3.9, "
                       "whose x86_64/Cargo.toml declares this package version. Reproduced from "
                       "the byte-identical cached winapi 0.3.9 license files, hash-checked by the generator. "
                       "libwinapi_oemlicense.a is an import library, not a license document.</p>"]
        root, files = notice_files(package)
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
    if windows:
        output += windows_notices()
    output += system_notices()
    output.append("</body></html>\n")
    return "\n".join(output)


def generate(windows=True):
    return markdown(generate_html(windows=windows)).replace(
        "Texts below are reproduced without modification (HTML-escaped).",
        "Texts below are reproduced without modification in verbatim fenced blocks.")


def check_darwin(target):
    """Check non-Linux target attribution without invoking Linux gcc/dpkg.

    Darwin system inputs are separately checked in build-provenance.py. Windows
    GNU runtime inputs still require native evidence and independent review;
    crate and Rust notices alone do not approve Windows binary distribution.
    """
    if target not in {"aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-pc-windows-gnu"}:
        raise ValueError("unsupported non-Linux notice target")
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
        _, files = notice_files(package)
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
    if target == "x86_64-pc-windows-gnu" and markdown("\n".join(windows_notices())).strip() not in text:
        raise ValueError("Windows GNU runtime attribution missing or changed")


def main():
    if NOTICE.is_symlink():
        raise ValueError("notice output must not be a symlink")
    if sys.argv[1:] == ["--write"]:
        NOTICE.write_bytes(generate().encode())
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
