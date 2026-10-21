# Local versioned release preparation

This process prepares reviewable versioned Linux/macOS assets locally and configures
the same gates for public CI. It does not implement GitHub release automation,
publish assets, create tags, push refs, run Actions, or install/activate the plugin.
**0.2.0 is released** for Linux/macOS. Windows 10/11 x86_64 is intended for
**0.3.0**, currently **PREPARATION only**, with the package version still **0.2.0**;
no new tag, merge, push or publication is approved by this preparation.
The historical `0.1.0` first regular Linux release was for
`x86_64-unknown-linux-gnu` only. On 2026-10-16, explicit user approval authorized
tag `v0.1.0` at reviewed source `490b0d74dc866b6ed83786f7aaa5657c35db5c51` and
publication as the regular [Latest release](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.1.0),
not a prerelease, with exactly four reviewed clean CI assets.

- [Fresh CI artifact verification](builds.md#regular-010-public-pipeline-evidence)
  and all four public-download hash comparisons passed. The local binary with
  personal build paths was not published; historical assets were not reused.
- [Public installation](installation.md#regular-010-public-installation) passed:
  release-tag-verified, source-SHA-pinned native navigation, plus separate unpinned
  CLI installation from merged main. No `0.1.0` second-runtime upgrade cycle was tested.
- PR #10 merged as `9ed72efed62672a1cee087bb9d3e353cce52276b`; its tree
  `a18018e4a56c60d7ba814e6e44a6de2d0a595db2` matches reviewed release source.
- Future commits, tags, pushes, CI execution and publication require their own
  approvals; this completed publication grants no standing authorization.

The completed historical public installation,
update and return test used `0.0.1 → 0.0.2 → 0.0.1`, with each version built and
checked independently. Test publications should be marked as GitHub prereleases;
their canonical versions remain stable `X.Y.Z`. A GitHub
prerelease flag does not change the canonical `X.Y.Z` version accepted by the tools.
Historical `0.1.0` evidence below is not proof for the new regular release, even
though the version matches. Fresh local and CI runs generated their own
build/map/archive evidence despite matching historical binary hashes from unchanged
toolchain/dependencies and Rust source/version; never rename or reuse historical assets as a new release.

On 2026-10-16, reviewed source `1610de9c969a487bb639881cd0c71fe1ee0e3ee2`
was published as the non-latest
[v0.0.1 prerelease](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.0.1)
with the exact four assets recorded in
`target/public-pipeline/38867597394/release-trial.json`. Its isolated Herdr 0.9.5
public CLI-install and pinned native single-version tests passed; all four public
downloads matched the approved hashes, and
[main CI run 38871442877](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38871442877)
passed. Reviewed source `c24df0876dae2a219bb833695829f7dc99433ae2` was subsequently
published as the retained non-latest
[v0.0.2 prerelease](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.0.2),
with exactly four [reviewed clean CI assets](builds.md#current-002-public-pipeline-evidence).
The local 0.0.2 binary contained personal build paths and was not published.
Both prereleases remain available with their own four assets. The actual
[public install/update/return cycle](installation.md#public-installupdatereturn-cycle)
passed with Herdr 0.9.5 on 2026-10-16: unpinned initial main
`24e60502de675ca1d6d501ae1cc03421276a03c4`, actual unpinned update to merged main
`80a15b34bbfc247b26f8f9737e8d84ba76a71e1d` (tree exactly matching reviewed 0.0.2
source), and pinned return to the 0.0.1 release SHA. The primary manually confirmed
the authorized transition only after initial navigation, maintaining the required
external main freezes. Native navigation, notices, no Rust/Cargo, config retention,
uninstall, owned shutdown and temporary cleanup all passed; receipts are
`target/public-cycle/acceptance.json` and `target/public-cycle/main-transition.json`.

Both required PR #8 checks passed, including the actual internal-positive source
policy run. The live fork-negative test was expressly waived, not executed.
The 0.0.2 Linux gates passed on attempt 2 after an unchanged transport fixture
failed with `ExecutableFileBusy` rather than expected `Other`; neither assertions
nor gates changed, and the root cause remains uncertain. The historical policy
behavior-RED timing gap and glibc DSC-signature limitation remain. Post-merge
[main CI run 38878662737](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38878662737)
passed in 4m 5s at exact merged commit `80a15b34bbfc247b26f8f9737e8d84ba76a71e1d`,
including formatting, Clippy, Rust/helper tests, the builder-profile gate, build,
all four full PTYs, packaging and upload. These are test-version results, not the
fresh gates or separate publication approval for the regular `0.1.0` release above.

## Windows release boundary

The Windows workflow/helper currently prepare evidence, not releasable or
certified assets. Both native Windows CI attempts reached the 30-minute job
timeout without uploading Windows preparation artifacts; the first also failed
its initial console-mode check before terminal entry. There is **no native Windows
GREEN**. The current fixture lifetime repair, diagnostics and watchdog changes
still await native execution. Versioned output names still use **0.2.0**;
do not relabel those files as 0.3.0 or infer a version bump
from the intended next release. See [the exact helper and artifact contract](builds.md#windows-preparation-not-certification).

Before a Windows release, require native pipeline success and exact artifact
verification, MSVC/SDK linked-input attribution review, Windows 10/11 runtime
certification, independent combined review and primary acceptance. The helper's
fake-host ConPTY/named-pipe and installer-byte checks are **not** actual Herdr
managed-install/live-popup integration. A physical Windows PC is available;
record its still-unknown OS version and perform final popup acceptance **only
after the pipeline stands**. This gate is pending, not waived. Host keybinding
setup is Unix-only and Windows uses `crossroads-windows`; reconcile those
[launch limits](installation.md#windows-preparation-limitations) before acceptance.
Version changes, remote actions and release/publication approval remain separate.

The 0.2.0 gate checklist and dated receipts below describe the earlier macOS
release preparation, not remaining Windows acceptance or authority to release again.

## Version and asset contract

`python3 scripts/check-version.py` reads the checkout's `Cargo.toml`, the unique
`herdr-crossroads` package entry in `Cargo.lock`, and `herdr-plugin.toml`. It prints
one agreed version or fails with no version output. Read/parse errors, missing
fields, duplicate/missing lock entries, mismatches, and non-string versions fail.
Only canonical stable `X.Y.Z` with ASCII digits and no leading zeroes is supported;
prereleases and build-metadata suffixes are deliberately excluded. These checks
prevent version data from becoming shell commands or traversing artifact paths.
The separate [binary installation hook](installation.md) reuses this checker
without invoking Rust/Cargo. It requires Python 3.11+ standard library only;
an explicit trusted local asset directory selects offline installation, otherwise
the prepared route downloads the exact version from GitHub Releases. Publication
of both test prereleases and execution of the full public cycle are recorded
above; this verified test cycle is not regular-release approval.

For `0.2.0`, each of `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, and
`x86_64-apple-darwin` produces four distinct assets. Linux example:

```text
target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.tar.gz
target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.tar.gz.sha256
target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.provenance.json
target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.link.map
```

New builds' tar contains the exact tested executable, preserving its executable mode,
`herdr-crossroads.sha256`, the project's `LICENSE`, and `THIRD-PARTY-NOTICES.md`.
The installer requires all four files and retains both notices beside the binary.
The already-published `v0.1.0` archive still contains `THIRD-PARTY-NOTICES.html`;
it is not replaced or relabeled by this conversion. New 0.2.0 artifacts are
separate; never replace v0.1.0 assets. When the checkout version is 0.1.0, the
installer accepts only the exact historical HTML paired with the pinned equivalent Markdown;
[installation](installation.md#validation-and-failure-behavior) specifies the hashes
and unchanged negative checks. Old pinned checkouts continue installing HTML.
Ownership is normalized to numeric root. Both
checksum entries use portable filenames. SHA256 detects corruption, not asset
authenticity or publisher identity. See [builds](builds.md) for prerequisites,
failure/stale-output behavior, extraction checks, and installation limitations.

## 0.2.0 manual release gates

1. Freeze the reviewed candidate source/tree and record its full commit identity.
   Run complete final Linux, native Apple Silicon and native Intel CI against that
   candidate, including packaging/provenance/notices and the real-Herdr isolated
   installation gate before upload. [Current candidate CI passed on all three
   targets](builds.md#current-020-three-platform-ci-evidence), with tested synthetic
   merge/tree identity recorded separately from the candidate head. Earlier green
   `6f91a3f` runs alone do not prove these later changes.
2. Independently inspect the exact uploaded archives, checksums, binary/map hash
   bindings and source identities for all three targets. Review Linux ELF/system
   provenance without dropping existing license evidence; review Darwin active
   Cargo notices, actual map inputs, SDK/toolchain/load commands, external system
   dylibs and ad-hoc signature evidence. Both Darwin assets must declare minimum
   macOS **15.0**; no Developer ID/notarization is provided. Publish only the exact
   tested binaries: never substitute a post-test signing/stripping build or stale
   assets from another source/version/run. The current downloaded assets passed
   [checksum/contract/hash and static Mach-O checks](builds.md#current-020-three-platform-ci-evidence); this is
   self-check evidence, not independent review approval.
3. Obtain independent combined Code/Tests/Docs review approval and final primary
   acceptance. The true macOS Herdr live-popup gate was explicitly **waived, not
   performed**; automated installation/distribution was **not waived**. Neither
   Rosetta execution nor public 0.2.0 asset downloading has been proven.
4. Separately approve each push, merge, tag and manual publication action. Record
   the release candidate/tag source identity separately from merged main's actual
   commit; a merge SHA may differ even when trees match. Require the final tested
   tree/asset identities to match the approved release; reconcile and recheck any
   substantive final source changes. No release automation or remote branch-rule
   change is part of this preparation.
5. Coordinate default-branch deployment with matching approved assets: the normal
   `herdr plugin install eisenhauer-dev/herdr-crossroads` checks out default HEAD
   and requests that checkout's version. Do **not** advance it to 0.2.0 with assets
   missing unless a separately approved controlled publication window handles the
   gap. Keep the pinned Linux fallback `--ref v0.1.0` explicit; there is no published
   macOS fallback. After approved publication, separately verify all public asset
   hashes and the public download/install route. Local-fixture CI cannot replace
   that later gate. Keep v0.1.0 and its historical evidence immutable.

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
python3 tests/test_pr_source_policy.py
python3 tests/test_version.py
python3 tests/test_build_linux.py
python3 tests/test_build_macos.py
python3 tests/test_build_provenance.py
python3 tests/test_install_binary.py
python3 tests/test_distribution_helpers.py
python3 tests/test_release_install_helpers.py
python3 tests/test_notices.py
sh scripts/build-linux.sh
(cd target/builds && sha256sum -c herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.tar.gz.sha256)
tar -tvzf target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.tar.gz
```

This block is the Linux preflight. On each native Mac use the corresponding
[macOS helper/prerequisites](builds.md#macos-020-preparation) instead of the Linux
build/archive commands; also check `tests/test_macos_workflow.py`.

The helper builds the locked optimized explicit-target binary and runs all four
full PTYs (latency, render, keymaps, hints) before packaging. Cargo tests' partial
keymap coverage is not a substitute. Review version agreement, source changes,
gate results, tar membership/mode/ownership, and both checksums before requesting
release approval. A file left by an earlier build is not fresh success evidence.
Run `sha256sum -c herdr-crossroads.sha256` after extraction into a separate empty
directory. Do not run concurrent builds in the same checkout.

The JSON/map sidecars are evidence outside the unchanged four-member tar and are
uploaded with the archive/checksum by CI using explicit versioned filenames.
They bind the final map to the exact tested binary, record tool/link command output,
and preserve installed package/source versions and file hashes where available.
See [builder evidence and limits](builds.md#builderlink-evidence-not-distribution-approval).
Review the normalized raw map rather than treating every traced file or GCC
candidate as an actual contribution. The actual PR run's system candidates and
exact Ubuntu package/source evidence are now recorded in the reviewed Ubuntu
profile below. The [fresh 0.0.1 CI gate and matching raw-map/ELF checks](builds.md#current-001-public-pipeline-evidence)
passed; repeat these checks for each new asset before distribution. A successful collection,
profile match, or build alone is not distribution approval.

## License inventory and verification

`THIRD-PARTY-NOTICES.md` preserves actual full license/copyright/attribution
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

The generator renders its reviewed HTML vocabulary into Markdown headings,
links, nested blockquotes (preserving the scope of Rust's in-tree exceptions),
and expanded notice sections rather than embedding the upstream HTML body.
Preformatted license text is HTML-decoded exactly once and fenced without
stripping or reflowing it; fences exceed any backtick run in the text. Cached
crate files are read as UTF-8 bytes, preserving their original CRLF where present
(the old HTML generator normalized those line endings). `.gitattributes` disables
Git text normalization for this generated file so checkouts retain those bytes
and the installer compatibility pin. Targeted `-text` rules also preserve
`LICENSES/*-system.json` and `LICENSES/glibc-*-startup.txt`: their exact bytes are
part of the checked notice provenance, including on `core.autocrlf=true` Windows
checkouts. Tests materialize a real Git index with that conversion enabled and
verify both preserved evidence and rejection/detection of changed evidence.
The upstream Rust HTML
SHA256 still identifies the original source bytes, not the Markdown rendition.
Tests compare all 458 preformatted blocks in order, all prose and external links,
and all 140 active packaged license/notice files against the exact local sources.
The historical renderer, with its original newline normalization, reproduces the
HTML hash from reviewed source
[`490b0d74dc866b6ed83786f7aaa5657c35db5c51`](https://github.com/eisenhauer-dev/herdr-crossroads/blob/490b0d74dc866b6ed83786f7aaa5657c35db5c51/THIRD-PARTY-NOTICES.html).
On 2026-10-16, the public `v0.1.0` archive and its adjacent checksum were read
and verified: archive SHA256
`d55b5bc611f74a7f1a61de789428e01fcf9b414640153c796f10a6a8a049304e`,
with the exact pinned HTML and checkout-matching `LICENSE`. This read-only check
does not replace a new native installation or distribution acceptance run.

For approved dependency/toolchain/builder changes, with exact cached crate sources,
the installed toolchain's Rust documentation, native GCC driver, `dpkg-query`,
and installed package copyright/common-license files matching one reviewed profile:

```sh
python3 scripts/third-party-notices.py --write
python3 scripts/third-party-notices.py --check
python3 tests/test_notices.py
```

Review the inventory and full-text diff; generation is not legal approval.
`--check` is the pre-build gate: it compares a deterministic parsed Cargo
manifest/lock fingerprint (excluding only this project's version), exact Rust
version, and actual installed system inputs against either reviewed builder.
Matching includes resolved GCC candidate paths/hashes, exact binary/source package
versions and complete installed copyright/common-license bytes. Both checked-in
profiles and their exact source evidence are mandatory; notice-text hashes and
source-evidence hashes are checked, and the canonical system section must equal
its checked-in-data rendering. Missing, changed or unreviewed inputs fail before
Cargo/PTYs; `--write` also refuses an unreviewed host rather than silently capturing
it. The two profiles are explicit, not a generic builder/plugin mechanism.

Generation renders both conservative labeled profiles in stable order, producing
the same canonical notice on either builder. It does not overwrite the reviewed
records from the current host. `test_notices.py` reproduces the entire notice from
cached crates, installed Rust notices and reviewed system data, and tests both
profile matches plus drift/missing/tampered evidence. Full regeneration requires
the installed Rust documentation; the CI build's mandatory `--check` does not.
Neither command downloads dependencies or proves attribution completeness or
newly linked ELF contributions. These are builder controls, not runtime distro
constraints. Reassess changed builders, custom license paths and inline attribution
requirements explicitly rather than assuming filename matching covers future inputs.

### Ubuntu 24.04 system profile

`LICENSES/ubuntu-24.04-system.json` records actual candidates from public PR run
[38812409902](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38812409902),
commit `47b5a5774eb6227809c222323668a6c1e5b35b2f`, artifact `12008823316`.
Its binary SHA256 is `63857e066f8e4c7821114107292e5e973aaab0b12144ab13b1a63e59611bace1`;
its normalized map SHA256 is `ef6bbe624753a8a54c94a0592f155c90fecfa0fdf5f66ebbd3271ccb5c3432b6`.
All 14 collected GCC candidates, including `crt1.o`, matched bytes extracted from
the exact official Ubuntu `.deb` packages. The canonical profile uses the same 13
startup/runtime queries as the local notice inventory (excluding `crt1.o`).

| Inputs | Binary/source package version |
| --- | --- |
| glibc startup/nonshared/development inputs and runtime/loader | `libc6-dev:amd64`, `libc6:amd64`; `glibc 2.39-0ubuntu8.9` |
| GCC CRT/static runtime/development linker inputs | `libgcc-13-dev:amd64`; `gcc-13 13.3.0-6ubuntu2~24.04.1` |
| Dynamic GCC runtime | `libgcc-s1:amd64`; `gcc-14 14.2.0-4ubuntu2~24.04.1` |

Exact package archive paths, sizes and SHA256 values are preserved in the profile;
downloads used `https://archive.ubuntu.com/ubuntu/` plus those paths. On
2026-10-15 all seven downloaded packages (including matching `gcc-13-base`,
`gcc-14-base` copyright providers and `base-files`) matched Ubuntu's official
`noble-updates/main/binary-amd64/Packages.xz`. That index matched its `InRelease`
hash; `gpgv` verified the latter with Ubuntu archive key
`F6ECB3762474EDA9D21B7022871920D1991BC93C` from the installed archive keyring.
The profile preserves all exact package copyright texts and referenced full common
licenses. Common-license donor `base-files 13ubuntu10.5` is **not** claimed to be
the runner's installed version: the gate checks those license bytes, not the donor
package version. None of these packages was installed for this verification.

[Exact Ubuntu glibc source evidence](../LICENSES/glibc-2.39-0ubuntu8.9-startup.txt)
records the DSC and both archive URLs/sizes/hashes, complete patched-file hashes
and all eight full exception headers. Archive sizes/hashes matched the exact
`glibc_2.39-0ubuntu8.9.dsc`; `dpkg-source --no-check -x` successfully applied the
entire Ubuntu patch series before the headers were extracted. The DSC signature
was not verified: HTTPS primary-source retrieval and matching DSC checksums are
the stated source-provenance boundary, distinct from signed binary-index checking.
As for the local profile, `csu/init.c` and `csu/static-reloc.c` have no such exception.
Do not extend the eight file-specific exceptions to those or other sources.

On 2026-10-16 the bound map and exact downloaded ELF from this run were checked
against the verified package objects and patched source. The map contains
`Scrt1.o` ABI-note, unwind and `_start` sections, `crti.o`/`crtn.o` init/fini
sections, and GCC CRT contributions; no static `libc_nonshared.a`, `libgcc.a` or
`libgcc_eh.a` contributions appear. The ELF and map lack `_IO_stdin_used`
(`csu/init.c` data) and `_dl_relocate_static_pie`. The patched `csu/Makefile`
identifies `Scrt1.o` inputs as `start.os`, `abi-note.o` and `init.o`; the unused
`init.c` data is discarded. This checks that run's actual startup contributions,
not arbitrary later builds or a universal glibc exception.

The retained local copy of the old PR artifact carries the old local-only notices; it is evidence for
its candidate bytes, not a corrected distribution asset. The fresh
[0.0.1 pipeline verification](builds.md#current-001-public-pipeline-evidence)
passed the new mandatory matching-profile gate, all four full PTYs and
packaging/upload, independent four-member tar/checksum/canonical-notice checks,
and matching raw-map/ELF reconciliation before the 0.0.1 publication;
these artifact-specific checks must be repeated for new assets.

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

The earlier local-only-notice four-member asset was rebuilt and verified on
2026-10-15, with all
four full PTYs and matching final binary/map evidence. Both isolated real-Herdr
0.9.5 harnesses passed; [current native evidence and hashes](installation.md#native-distribution-acceptance)
are distinct from the retained historical two-member evidence. This remains
local preparation, not publication or final release approval. Those native results
precede dual-profile notice regeneration. The
[subsequent local native acceptance](installation.md#dual-profile-local-acceptance)
passed against the repackaged dual-profile asset. Public acceptance was still
pending at that historical checkpoint; the later real cycle is recorded above.

## Public CI and later publication remain separate

The existing **Linux gates** job accepts `push`, `pull_request`, and manual
`workflow_dispatch` on an existing ref. Every invocation remains public-only,
read-only, and uses the same gates and helper. Only successful packaging writes
the exact versioned name to step outputs; upload selects its tar, archive
checksum, provenance JSON and link map, never a `target/builds/` wildcard. There is no tag trigger or release
publication job.

The helper-test step includes `python3 tests/test_pr_source_policy.py`,
`python3 tests/test_version.py`,
`python3 tests/test_build_linux.py`, `python3 tests/test_build_provenance.py`,
`python3 tests/test_install_binary.py`, and
`python3 tests/test_distribution_helpers.py`,
also present in the local preflight list above. Full notice reproduction is an
additional local-builder check, not an existing CI helper step. Installer unit tests use synthetic assets
and isolated destinations, not a live installation or real Herdr integration.
Distribution helper tests validate fixture preparation and current PTY cells
without launching Herdr; the historical live-server opt-in harness remains outside CI.
The new `tests/test_release_install_helpers.py` checks isolated fixture helpers;
CI additionally runs `tests/test_release_install.py` with pinned Herdr 0.9.5 and
the exact packaged archive before upload. This actual managed install/reinstall,
negative/config-retention and installed-binary four-PTY gate uses local Git/assets
and no server/popup. Its native execution passed on all three targets in the
[current candidate runs](builds.md#current-020-three-platform-ci-evidence), separately
from the historical public runs below. The macOS workflow runs its four full PTY
gates on the fresh final-linked binary inside packaging, each with a five-minute
watchdog, rather than also testing a separate ordinary release build. Real
installation acceptance still precedes exact-asset upload. Both workflows remain
public-only/read-only with manual publication separate.

The public [main run on 2026-10-15](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38775076190)
passed the gates, explicit-target build, all four full isolated PTYs, packaging,
and upload on Ubuntu 24.04. This is not a dry-run release publication.
Independent [downloaded pipeline-artifact verification](builds.md#downloaded-public-pipeline-verification)
passed on 2026-10-15, including all four full local PTYs against the downloaded
binary. The later [0.0.1 public pipeline](builds.md#current-001-public-pipeline-evidence)
passed the new Ubuntu builder gate and matching system/link provenance checks.
The [release-procedure local trial](installation.md#pipeline-asset-local-trial)
passed using that public CI artifact, not a released asset or public download-mode
installation. The later publications of both test prereleases and the completed
real public cycle are recorded above. Before any new tag, push, or publication, obtain
explicit approval for that action and the reviewed version/ref/assets. A proposed
tag convention is `v<version>` (for example `v0.0.2`); no tag is created here.

Local Linux x86_64 evidence uses glibc 2.43. The public CI run separately verifies
its Ubuntu 24.04 isolated test environment, not live Herdr integration there,
a minimum glibc version, or another distribution. The separate
[native Herdr 0.9.3 local managed lifecycle test](installation.md#native-distribution-acceptance)
passed on 2026-10-14, including replacement and return with a test-only version
fixture. The earlier local-only-notice four-member asset passed both 0.9.5 native harnesses on
2026-10-15, including notice retention and version-specific registration/uninstall
checks. Neither run proves new-runtime upgrade compatibility or the separate
Herdr update command. The separate pinned public 0.0.1 tests passed as recorded above;
the later full public cycle passed separately. Those historical tests did not
authorize the regular release; its separate approval and publication are recorded
at the start of this guide. No GitHub release automation is implemented.
The opt-in real-Herdr sandbox test is main-owned, not a routine release command.
The public source is `eisenhauer-dev/herdr-crossroads`; public repository
access was verified on 2026-10-15. Public CI execution and upload are verified;
0.0.2 tag/assets and the actual public cycle were subsequently verified as recorded
above. Neither the verified test cycle nor the pipeline-asset local trial constitutes
regular-release approval.
Downloaded pipeline-artifact verification is
recorded in the build guide; it is not a released-asset installation test.
