# Local versioned release preparation

This process prepares reviewable versioned Linux assets locally and configures
the same gates for public CI. It does not implement GitHub release automation,
publish assets, create tags, push refs, run Actions, or install/activate the plugin.
The current version remains `0.1.0`; no version bump is part of this preparation.

## Version and asset contract

`python3 scripts/check-version.py` reads the checkout's `Cargo.toml`, the unique
`herdr-crossroads` package entry in `Cargo.lock`, and `herdr-plugin.toml`. It prints
one agreed version or fails with no version output. Read/parse errors, missing
fields, duplicate/missing lock entries, mismatches, and non-string versions fail.
Only canonical stable `X.Y.Z` with ASCII digits and no leading zeroes is supported;
prereleases and build-metadata suffixes are deliberately excluded. These checks
prevent version data from becoming shell commands or traversing artifact paths.
The separate [local-asset installation hook](installation.md) reuses this checker
without downloading or invoking Rust/Cargo. It requires Python 3.11+ standard
library only and an explicit trusted local asset directory.

For `0.1.0`, the only supported target produces:

```text
target/builds/herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz
target/builds/herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
```

The tar contains the exact tested executable, preserving its executable mode,
`herdr-crossroads.sha256`, the project's `LICENSE`, and `THIRD-PARTY-NOTICES.html`.
The installer requires all four files and retains both notices beside the binary.
Ownership is normalized to numeric root. Both
checksum entries use portable filenames. SHA256 detects corruption, not asset
authenticity or publisher identity. See [builds](builds.md) for prerequisites,
failure/stale-output behavior, extraction checks, and installation limitations.

## Local preflight and review

From the repository root, with already-installed tooling and cached dependencies:

```sh
export CARGO_NET_OFFLINE=true
python3 scripts/check-version.py
python3 tests/test_ci_workflow.py
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 tests/test_git_hooks.py
python3 tests/test_version.py
python3 tests/test_build_linux.py
python3 tests/test_install_binary.py
python3 tests/test_distribution_helpers.py
python3 tests/test_notices.py
sh scripts/build-linux.sh
(cd target/builds && sha256sum -c herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256)
tar -tvzf target/builds/herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

The helper builds the locked optimized explicit-target binary and runs all four
full PTYs (latency, render, keymaps, hints) before packaging. Cargo tests' partial
keymap coverage is not a substitute. Review version agreement, source changes,
gate results, tar membership/mode/ownership, and both checksums before requesting
release approval. A file left by an earlier build is not fresh success evidence.
Run `sha256sum -c herdr-crossroads.sha256` after extraction into a separate empty
directory. Do not run concurrent builds in the same checkout.

## License inventory and verification

`THIRD-PARTY-NOTICES.html` preserves actual full license/copyright/attribution
texts, not just SPDX expressions, including system startup/runtime package records
and the full system license files those records reference.
`scripts/third-party-notices.py --write` uses
`cargo tree --offline --locked --target x86_64-unknown-linux-gnu -e normal,build`
and offline, locked Cargo metadata for the same target. The current inventory
covers **78 unique active registry package versions**, including both `syn`
and `hashbrown` versions. Each package records exact name/version, crates.io
provenance, Cargo.lock checksum, upstream license expression, and all packaged
license, NOTICE, copyright and attribution files found by name recursively.
No packages are downloaded. The conservative whole normal/build graph includes
build-only/proc-macro dependencies; it is not a claim that every entry is linked.
Dev-only and inactive-target dependencies are excluded.

The exact cached manifests identify `jsonc-parser 0.34.0` and `ratatui 0.30.2` as
MIT, `serde_json 1.0.151` as MIT OR Apache-2.0, and
`unicode-general-category 1.1.0` as Apache-2.0. `foldhash 0.2.0` has Zlib terms;
`unicode-ident 1.0.26` additionally requires Unicode-3.0. Their actual upstream
texts, including copyright notices, are preserved. No single license selection
is inferred from a multi-license expression.

Rust's standard library is outside Cargo.lock. The notice also reproduces the
installed **Rust 1.99.0**, commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`,
`share/doc/rust/COPYRIGHT-library.html` in full, recording its SHA256. This upstream
inventory includes 141 out-of-tree package entries and cross-target/build material;
it is deliberately conservative, not a Linux link map.
Rust's exact-commit [COPYRIGHT](https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/COPYRIGHT)
identifies this file as the standard-library attribution source. The public
versioned documentation URL returned 404 on 2026-10-15, so the installed
toolchain's notice file was used with explicit read authorization.

For approved dependency/toolchain/builder changes, with exact cached crate sources,
the installed toolchain's Rust documentation, native GCC driver, `dpkg-query`,
and installed package copyright/common-license files available:

```sh
python3 scripts/third-party-notices.py --write
python3 scripts/third-party-notices.py --check
python3 tests/test_notices.py
```

Review the inventory and full-text diff; generation is not legal approval.
`--check` is the cheap build gate: it compares a deterministic parsed
Cargo manifest/lock fingerprint (excluding only this project's version) and
exact Rust version. It does not regenerate or prove attribution completeness.
`test_notices.py` reproduces and compares the entire notice using the cached
sources, installed Rust notices, system object hashes, package/source versions,
and complete installed system notices/licenses. Run it on the actual packaging
builder before release; it detects builder drift without hardcoding this host's
glibc/GCC versions. `--check` intentionally does not verify system provenance or
the newly linked ELF. Regeneration's Debian/Ubuntu package-discovery procedure
has only been exercised on this local builder. Its checked-in glibc source-header
evidence must match the installed source package/version; generation rejects a
mismatch rather than extending exceptions to other versions. A different builder
needs its own reviewed source evidence, not a new runtime distro constraint.
This is not evidence for Ubuntu 24.04. It is a focused local check, not added to
the existing CI helper-test step by this change. Neither command installs tools
or downloads dependencies. Reassess new custom license paths/inline attribution
requirements rather than assuming filename matching covers future packages.

### Installed system provenance and verified startup exceptions

Read-only local investigation on 2026-10-15 established these exact installed
binary/source package versions through `gcc -print-file-name`, resolved paths,
`dpkg-query -S`, and `dpkg-query -W`:

| Inputs | Binary package | Source package/version |
| --- | --- | --- |
| `Scrt1.o`, `crti.o`, `crtn.o`, `libc.so` linker script, `libc_nonshared.a` | `libc6-dev:amd64` | `glibc 2.43-2ubuntu2.4` |
| GCC CRT objects, `libgcc.a`, `libgcc_eh.a`, `libgcc_s.so` linker input | `libgcc-15-dev:amd64` | `gcc-15 15.2.0-16ubuntu1` |
| `libc.so.6`, `ld-linux-x86-64.so.2` | `libc6:amd64` | `glibc 2.43-2ubuntu2.4` |
| Dynamic `libgcc_s.so.1` | `libgcc-s1:amd64` | `gcc-16 16-20260322-1ubuntu1` |

The binary package versions equal their source versions above. In particular,
the dynamic GCC runtime is **not** version 15.2.0 merely because the compiler is.
The artifact records resolved paths and SHA256 for all candidate inputs, full
copyright records from `/usr/share/doc/{libc6,libc6-dev,gcc-15-base,gcc-16-base}/copyright`,
their provider package versions, and full referenced `/usr/share/common-licenses/`
texts, including LGPL-2.1 and GPL-3. The common-license provider is
`base-files 14ubuntu6.2`; GCC notice providers match their respective GCC source
versions. Package-wide records also describe unrelated languages/architectures;
they are preserved conservatively, not asserted to be linked.

GCC's installed record explicitly assigns libgcc and `crtstuff.c` (the source
named in `crtbeginS.o`/`crtendS.o`) GPL-3-or-later with Runtime Library Exception
3.1. Its full exception is delivered. Section 1 permits eligible independently
compiled combinations under their own terms; this does not waive requirements
for arbitrary GPL code. The documented Rust/LLVM compilation and native GCC
linking route fits its eligible-process definition when no proprietary GCC
intermediate-representation optimizer is used; reassess a changed compilation route.

The ELF is PIE; its comments identify Rust 1.99.0, Ubuntu GCC
15.2.0-16ubuntu1, and LLD 23.1.1. On 2026-10-15 a fresh link produced
`target/builds/herdr-crossroads-link.map`, SHA256
`b357010a49224ddc4e09a1510c814775f9b7f6677ffbf8a8ef87ea241b8d6052`.
The mapped executable is byte-identical to the release-gate executable, SHA256
`146d556e5afee59c61b0664b85457b16b31764d0662062ce553edc4374248c1c`.
The map records contributions from `Scrt1.o`, `crti.o`, `crtn.o`,
`crtbeginS.o` and `crtendS.o`; no `libc_nonshared.a` member contributes.
This is fresh local link evidence, not a recovered historical map or a result
for another builder. The diagnostic command was:

```sh
cargo rustc --offline --locked --release --bin herdr-crossroads \
  --target x86_64-unknown-linux-gnu -- \
  -C "link-arg=-Wl,-Map=$PWD/target/builds/herdr-crossroads-link.map"
```

The ELF dynamically needs `libc.so.6`, `libgcc_s.so.1`, and the loader, with no
RPATH/RUNPATH. These shared libraries are not bundled; no source offer is being
made for copies this asset does not distribute. That fact does **not** exempt
startup/static code incorporated into the executable.

The earlier unavailable-header blocker is resolved by exact Ubuntu primary-source
evidence. [The checked-in startup notice data](../LICENSES/glibc-2.43-2ubuntu2.4-startup.txt)
preserves all eight full copyright/license/exception headers, complete-source
SHA256 values, archive URLs/sizes/hashes, and the patch-review method. The source
archives came from `https://archive.ubuntu.com/ubuntu/pool/main/g/glibc/`:

- `glibc_2.43.orig.tar.xz`: 20,297,012 bytes, SHA256
  `d9c86c6b5dbddb43a3e08270c5844fc5177d19442cf5b8df4be7c07cd5fa3831`.
- `glibc_2.43-2ubuntu2.4.debian.tar.xz`: 530,764 bytes, SHA256
  `28103a7cf808c29901c6053d89a4e8299880abfbb850c0d97a09c8df8533e306`.

Both archive sizes/hashes were independently checked against the exact
`glibc_2.43-2ubuntu2.4.dsc`; selected extracted files matched their original
archive members byte-for-byte. All **103 active patches** in the exact Debian
series were inspected through their unified/git diff path headers: none changes
the eight selected sources or `csu/init.c`/`csu/static-reloc.c`. Therefore this
series does not remove the selected exceptions. This was a source/path review,
not a glibc rebuild or an executed full patch application.

The full headers of `sysdeps/x86_64/{start,crti,crtn}.S`, `csu/abi-note.c`,
`stdlib/{atexit,at_quick_exit}.c`, `sysdeps/pthread/pthread_atfork.c`, and
`debug/stack_chk_fail_local.c` explicitly give unlimited permission to link their
compiled versions with other programs and distribute those programs without
restriction coming from use of those files. The latter four cover every member
of the installed `libc_nonshared.a`. The generator embeds the checked-in data
verbatim (HTML-escaped), checks its source-package/version against installed
`Scrt1.o` ownership metadata, and requires no temporary source directory or
network access. No source/relinking offer is needed for combinations covered by
these file-specific exceptions; they do not exempt arbitrary glibc code.

**Local startup-source gate passed:** the matching map contains only the covered
`Scrt1.o` start/unwind and ABI-note sections, plus the covered init/fini sections
from `crti.o`/`crtn.o`. Neither the map nor the final ELF symbol table contains
`_IO_stdin_used` or `_dl_relocate_static_pie`; their uncovered source units
`csu/init.c` and `csu/static-reloc.c` do not contribute code/data to this artifact.
The GCC CRT contributions are covered by the documented Runtime Library
Exception. Retain and repeat this final binary/map comparison on the actual
packaging builder; notice-generator tests alone do not establish it. Repack and
reverify the four-member archive whenever notices change, and reassess if
libraries are bundled, source versions change, or linking changes. No universal
clearance or other-platform execution result is established by this evidence.

The current four-member asset was rebuilt and verified on 2026-10-15, with all
four full PTYs and matching final binary/map evidence. Both isolated real-Herdr
0.9.5 harnesses passed; [current native evidence and hashes](installation.md#native-distribution-acceptance)
are distinct from the retained historical two-member evidence. This remains
local preparation, not publication or final release approval.

## Public CI and later publication remain separate

The existing **Linux gates** job accepts `push`, `pull_request`, and manual
`workflow_dispatch` on an existing ref. Every invocation remains public-only,
read-only, and uses the same gates and helper. Only successful packaging writes
the exact versioned name to step outputs; upload selects just its tar and
checksum, never a `target/builds/` wildcard. There is no tag trigger or release
publication job.

The helper-test step includes `python3 tests/test_version.py`,
`python3 tests/test_build_linux.py`, `python3 tests/test_install_binary.py`, and
`python3 tests/test_distribution_helpers.py`,
also present in the local preflight list above. Full notice reproduction is an
additional local-builder check, not an existing CI helper step. Installer unit tests use synthetic assets
and isolated destinations, not a live installation or real Herdr integration.
Distribution helper tests validate fixture preparation and current PTY cells
without launching Herdr; the native opt-in harness remains outside CI.

Prepared CI release-preflight is not an executed CI run or a dry-run publication.
Public CI and downloaded pipeline-artifact verification require a later public
run and their own remote approvals. Before any tag, push, or publication, obtain
explicit approval for that action and the reviewed version/ref/assets. A proposed
tag convention is `v<version>` (for example `v0.1.0`); no tag is created here.

Local Linux x86_64 evidence uses glibc 2.43. It does not prove the intended Ubuntu
24.04 CI runtime, a minimum glibc version, or another distribution. The separate
[native Herdr 0.9.3 local managed lifecycle test](installation.md#native-distribution-acceptance)
passed on 2026-10-14, including replacement and return with a test-only version
fixture. The current four-member asset passed both 0.9.5 native harnesses on
2026-10-15, including notice retention and version-specific registration/uninstall
checks. Neither run proves new-runtime upgrade compatibility or the separate
Herdr update command. The local-only installer is separate from public distribution;
no public installer readiness or actual GitHub release automation is claimed.
The opt-in real-Herdr sandbox test is main-owned, not a routine release command.
The selected future source is `eisenhauer-dev/herdr-crossroads`; public repository
access was verified on 2026-10-15. Release tag/asset availability,
run/upload/download evidence and final release approval remain open.
