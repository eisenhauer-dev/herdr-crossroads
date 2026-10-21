# Build artifacts and verification

The Linux helper retains `x86_64-unknown-linux-gnu`; the macOS helper prepares
`aarch64-apple-darwin` and `x86_64-apple-darwin`. Artifacts are version-named for local release
preparation; building does not publish releases. The build helper makes no tags, publication,
plugin activation, or installation changes. See [release preparation](releases.md)
for the version contract and separate approval/verification boundaries.

**0.2.0 is released** for Linux and macOS. Native Windows 10/11 x86_64 is intended
for **0.3.0**, currently **PREPARATION only**; the checkout's package/manifest
version remains **0.2.0**. The historical `0.1.0` Linux release was published as
[Latest](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.1.0)
on 2026-10-16 with exactly four reviewed clean CI assets, not a prerelease.
Historical local/CI `0.1.0` artifacts below were not reused or renamed.

## Windows preparation (not certification)

The WIP target is `x86_64-pc-windows-msvc`. Native Windows CI **has not run**.
The public-only `.github/workflows/windows.yml` selects the standard free public
`windows-2025` runner, PowerShell, Rust/Cargo 1.99.0, and Python 3.11+. It has
read-only contents permission, pinned checkout/upload actions, no persisted Git
credentials, and no release publication job. It checks formatting and native
library/binary Clippy before invoking the preparation helper.

On a native Windows x86_64 builder with Rust/Cargo 1.99.0, Python 3.11+, Git,
MSVC linker/Windows SDK, installed Rust documentation and cached locked
dependencies, the repository-root command is:

```powershell
python -B scripts/build-windows.py
```

This command is source-verified only, **not executed on Windows**. The helper
installs no tooling. It checks versions and target notices, forces a fresh final
link with an MSVC map and library diagnostics, validates PE bytes, and binds the
binary/map SHA256 in preparation-only provenance. It runs selected Windows,
input, search and navigation Rust tests, fake-host named-pipe/ConPTY acceptance,
and distribution checks. It packages exactly `herdr-crossroads.exe`,
`herdr-crossroads.exe.sha256`, `LICENSE`, and `THIRD-PARTY-NOTICES.md`, then runs
the Python installer in a temporary checkout, compares installed bytes with the
final-linked executable, and repeats fake-host acceptance on those installed bytes.
It does **not** run an actual Herdr managed install, server, client or live popup.

Outputs under `target/builds/` currently use
`herdr-crossroads-0.2.0-x86_64-pc-windows-msvc` with `.tar.gz`, `.tar.gz.sha256`,
`.provenance.json`, and `.link.map` suffixes. They follow the unchanged package
version, not the intended 0.3.0 release. CI uploads exactly these four files as a
`windows-preparation-` artifact with seven-day retention; these are not approved
release assets. Handled failures after output selection remove the selected
sidecars/archive; stale files never establish a successful invocation.

**Open gates:** native pipeline execution and exact artifact verification;
MSVC/SDK linked-input attribution review (a map/diagnostic capture is not license
clearance); Windows 10/11 runtime certification; actual Herdr popup acceptance;
independent review and separate release approval. A physical Windows PC **is
available**, but its OS version is not yet known. Final popup acceptance on that
PC is scheduled **only after the pipeline stands**, not waived or already passed.
Neither the runner label nor synthetic PE tests establish Windows 10/11 support.
See [installer and launch limitations](installation.md#windows-preparation-limitations)
and [native test scope](keymaps.md#windows-preparation-verification).

## macOS 0.2.0 preparation

Both Apple Silicon and Intel require **macOS 15.0+**. From the repository root on
each native architecture, `sh scripts/build-macos.sh` selects the corresponding
explicit Rust target and sets `MACOSX_DEPLOYMENT_TARGET=15.0`. It requires native
Rust/Cargo **1.99.0**, Python 3.11+, a POSIX shell, `stty`, Git, Apple command-line
tools/SDK (`xcrun`, clang, `lipo`, `otool`, `codesign`), cached locked dependencies
and Rust documentation, and the owned writable `/tmp/opencode` PTY directory.
It installs no tooling. Rosetta is rejected as a release builder.

The helper validates versions/notices, forces a fresh final link, verifies the
binary/evidence hash binding, then runs all four full PTYs against that exact
explicit-target executable, in latency/render/keymaps/hints order. Each gate has
a 300-second timeout; timeout or interruption kills its process group, and timeout
diagnostics retain captured output. CI relies on these final-binary gates instead
of a duplicate ordinary release build/PTY pass, then retains the separate
installed-package real-Herdr acceptance before upload. The job limit remains
30 minutes. Link-time ad-hoc signing occurs **before** testing;
there is no post-test signing, stripping or other binary mutation. These are
**not Developer ID signed or notarized** assets; ad-hoc signing and SHA256 do not
authenticate a publisher or establish Gatekeeper acceptance.

For each target, output under `target/builds/` uses
`herdr-crossroads-0.2.0-<target>` with `.tar.gz`, `.tar.gz.sha256`,
`.provenance.json`, and `.link.map` suffixes. The archive has the same four-member
contract as Linux, normalized root ownership and modes 0755 (binary)/0644 (others).
Handled build, notice, inspection, PTY or packaging failures remove the selected
version's outputs; unrelated versions remain. Never infer success from stale files.

Darwin provenance records the actual Apple link map/object inventory, link command
and diagnostics, native OS/toolchain/SDK versions, `lipo`/`otool` inspections,
ad-hoc signature display/verification, and bound binary/map hashes. The collector
preserves opaque non-UTF-8 Apple symbol bytes while normalizing repository paths;
the recorded map hash covers the resulting bytes. Object validation remains strict.
The collector
requires a thin native executable with `LC_BUILD_VERSION` minimum **15.0**, accepts
only project/dependency/Rust-library map inputs, linker-synthesized entries,
and Rust's generated `release/deps/rustcXXXXXX/symbols.o` when its canonical
target-specific path also appears in the same link's trace. The temporary suffix
must be six ASCII alphanumeric characters; other nested objects remain rejected.
Rust generates this symbol-reference object in
[`link.rs` at the toolchain source commit](https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/compiler/rustc_codegen_ssa/src/back/link.rs#L2621-L2626).
SDK `.tbd` entries are recorded separately in `sdk_stub_inputs`, with canonical
paths, SHA256 hashes and matching trace paths from the same link. These dynamic
interfaces must resolve inside the selected SDK and have UTF-8 TAPI v4 signatures;
this is not a full YAML validator. Unknown formats, binary impostors and escaping
symlinks fail closed. The distinction follows the
[LLVM 20.1.8 text-stub format](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/TextAPI/TextStub.cpp#L153-L156):
SDK interfaces are not statically incorporated implementation code.
The collector allows only the external OS dylibs `/usr/lib/libSystem.B.dylib` and
`/usr/lib/libiconv.2.dylib`. Unknown static
inputs, dylibs or map layouts fail for review. OS dylibs/SDK are not bundled;
these records are not a universal attribution or licensing verdict.

Both native architectures at `fdb0988` reported exactly these install names in
[run 39263352762](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39263352762).
The locked [libc 0.2.189 Apple declarations](https://github.com/rust-lang/libc/blob/0.2.189/src/unix/bsd/apple/mod.rs#L5251-L5265)
explicitly link `iconv` outside `rustc-dep-of-std`. This is an existing native
dependency, not a new bundled library. Apple's published
[libiconv-107 metadata](https://github.com/apple-oss-distributions/libiconv/blob/libiconv-107/libiconv.plist)
and [license texts](https://github.com/apple-oss-distributions/libiconv/blob/libiconv-107/libiconv.txt)
identify BSD terms, not GNU libiconv's LGPL terms. That source tag is not a verified
revision match for the runner's OS library. No OS library implementation is included
in the four-member archive; the static-input gate remains unchanged. The existing
Cargo/Rust notices therefore remain unchanged rather than attributing an unverified
Apple source revision as bundled code.

`third-party-notices.py --check --target <target>` checks the actual offline,
locked normal/build Cargo graph against the existing full-text notices and Rust
standard-library attribution. Linux system profiles remain conservatively retained;
Darwin does not query GCC/dpkg or pretend those Linux objects were linked.
The final map and SDK/system-library evidence still require artifact-specific review.

## Current 0.2.0 three-platform CI evidence

On 2026-10-19, [Linux run 39279008880](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39279008880)
and both native Apple Silicon/Intel jobs in [macOS run 39279009119](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39279009119)
passed every step, including the full PTYs, native packaging, isolated real-Herdr
0.9.5 install/reinstall and negative/config-retention checks, all four full PTYs
against each installed binary, and exact four-asset uploads. Each packager tests
its exact fresh final-link binary with all four full PTYs; macOS additionally
retains the four original full-PTY steps. These results supersede the distribution
evidence gap after the earlier `6f91a3f` PTY-only checkpoint. Installation used
local fixture Git/assets through actual Herdr 0.9.5, not published 0.2.0 downloads
or a real server/popup; see [installation scope](installation.md#automated-release-installation-gate).

The candidate head is `66c191e36f8733b4374b51c6b785f7bc6e09c109`. GitHub tested
the PR's synthetic merge `6aed9fa94d5b0881749d7f71afa0bdd86d5e6f86`, with parents
`5867263348523490a9da20e9cec825c428926fd0` and the candidate head. The owning session
verified via GitHub's Git commit API that its tree
`689ffa760bce752101ac04293d4a456a46303af6` matches the local candidate head's tree.
Both Darwin provenance files record the synthetic merge commit and clean source
status: this is matching-tree evidence, **not a claim that the head SHA was
directly built**, nor a prediction or authorization of a future merge SHA.

The owning session downloaded all three targets' four-asset sets under
`target/ci-evidence/66c191e/{linux,macos}/`. Standard-library verification passed
external archive SHA256, the four-member contract/internal binary checksum,
provenance binary/map hash bindings, and byte equality of `LICENSE` and
`THIRD-PARTY-NOTICES.md` with the checkout. Independent Mach-O byte parsing
confirmed each native CPU, macOS minimum **15.0**, exactly the two permitted
system dylib install names above, and a code-signature load command. Native CI's
signature verification reports valid-on-disk ad-hoc signatures; static checks on
the Linux host are not native execution or native signature verification.

| Target | Binary SHA256 | Link-map SHA256 |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `bfe9106604d5ca2ac6847fd8ea538997bb9fc81ef13422a5ddc451f66d9e9ab0` | `a2359b544cecdcc59aeb1a311e8df2322ac66d730260aa5d47101018f6a83134` |
| `aarch64-apple-darwin` | `2f5ea68077fea16f208f7b736931abb03959fc7a756c03eaa7a3287e234c5981` | `99fcccb56edcf264e706d5886c812adc109591b31ae8fb390b7bf0de1a6aeec4` |
| `x86_64-apple-darwin` | `e1cf8526820fc24413ce26e51f5b2f8742f06848f0a1c8c8e40ba0d104971e70` | `33b25d6dd5e2749e2bd80bd2ffd543707ff0be623cfbb7e789a1d837f73d6f79` |

Both Darwin records use Xcode 16.4 / SDK 15.5 and report macOS 15.7.9 (24G830),
with 15 ARM64 and 16 Intel SDK stub records. This is not a macOS-version test
matrix or an exact Apple OS-library source-revision match; the libc/libiconv
attribution limits above remain. Independent combined review has **not** been
performed and final acceptance is pending. True macOS Herdr live-popup acceptance
is **waived, not passed**; automated distribution was not waived. No Rosetta
execution, Developer ID signing/notarization or public 0.2.0 download is proven.
These are historical pre-release evidence limits; **0.2.0 was subsequently
released**. They do not authorize a new release, tag, merge or publication, and
none of this evidence certifies the Windows WIP. Published 0.1.0 remains immutable
and Linux-only.

## Build and verify locally

Run from a trusted checkout on native Linux x86_64. Have Rust/Cargo, a system
GCC/linker and Debian/Ubuntu `dpkg-query`, the already-installed
`x86_64-unknown-linux-gnu` Rust target, Python 3.11+,
`stty`, a POSIX shell, GNU tar, and `sha256sum` available. The PTYs require an
existing owned writable `/tmp/opencode` directory; they use fake Herdr executables
and sockets, not a live Herdr session.

From the repository root:

```sh
python3 scripts/check-version.py
python3 tests/test_version.py
python3 tests/test_build_linux.py
python3 tests/test_build_provenance.py
sh scripts/build-linux.sh
```

For a network-free build with already-cached locked dependencies, use
`CARGO_NET_OFFLINE=true sh scripts/build-linux.sh`. The helper does not install
tooling or Rust targets. Formatting, Clippy, Cargo tests, and hook checks remain
separate gates; this command does not replace them.

Before building, the helper checks that `Cargo.toml`, the `herdr-crossroads`
package entry in `Cargo.lock`, and `herdr-plugin.toml` agree on a canonical stable
`X.Y.Z` version. Missing, malformed, mismatched, or unsafe version data stops before
Cargo or the PTYs; prerelease and build-metadata suffixes are not supported.
It also checks the checked-in notice inventory's manifest/lock fingerprint and
exact `rustc --version`, then requires an exact match to one of the two reviewed
system profiles under `LICENSES/`: local glibc 2.43 or actual Ubuntu 24.04 CI.
Resolved GCC candidate paths, object hashes, binary/source package versions and
full installed copyright/common-license bytes must match. Both profiles and their
matching source-header evidence must be present and internally hash-consistent;
the canonical notice's system section must reproduce exactly from those records.
Dependency, feature, toolchain, or builder changes require notice
review and regeneration; changing only the project's version does not. Missing
license files or stale inventory stop before Cargo and remove current-version
artifact outputs. See [notice maintenance and final link-map verification](releases.md#license-inventory-and-verification).

The helper rejects other native operating systems/architectures, fixes Cargo's
output directory to this checkout's `target/`, and builds with
`cargo rustc --release --locked --bin herdr-crossroads --target x86_64-unknown-linux-gnu`
with binary-only linker map/trace/version arguments and `--print link-args`. It removes the
previous artifact files for the agreed version and target binary before building, then runs
`pty_latency.py`, `pty_render.py`, `pty_keymaps.py`, and `pty_hints.py` in that
order against exactly `target/x86_64-unknown-linux-gnu/release/herdr-crossroads`.
The complete keymap gate is used, never `--input-protocol`.

Only after all four full PTYs pass does it package the archive. A successful
invocation leaves these ignored outputs (JSON/map are collected before the PTYs):

```text
target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.tar.gz
target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.tar.gz.sha256
target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.provenance.json
target/builds/herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.link.map
```

These names use the checkout version `0.2.0`, not a fixed literal in the
helper. Other versions' outputs are left untouched and never selected as this
invocation's artifacts. The helper does not write `GITHUB_OUTPUT` or other workflow
metadata into caller-selected files.

The tar contains exactly four regular files: `herdr-crossroads` (with its executable
mode preserved), `herdr-crossroads.sha256` (the binary's SHA256), `LICENSE`
(the project's full MIT terms), and `THIRD-PARTY-NOTICES.md` (full dependency
license/attribution texts, Rust standard-library notices, and installed system
startup/runtime copyright/full-license records and exact glibc startup linking
exception headers). Keep the notices
with redistributed binaries. Archive ownership is normalized
to numeric root, rather than recording the builder's username. The adjacent file
checksums the tar. Checksum entries contain filenames, not checkout paths.
Checksums detect corruption, not authenticity or publisher identity.

This describes new local/CI builds, not a replacement of the published `v0.1.0`
assets, which still contain `THIRD-PARTY-NOTICES.html`. They remain immutable;
new 0.2.0 outputs are separate artifacts.
Do not publish or replace assets without separate approval. The current installer
supports that exact historical HTML through the
[pinned compatibility contract](installation.md#validation-and-failure-behavior).

Build, PTY, and packaging failures return nonzero and remove the current-version archive,
archive checksum, JSON/map evidence and temporary packaging files; handled INT/TERM
interruptions do likewise. A failed invocation never advertises success or reuses an old binary
as its new build. Do not run concurrent helper/Cargo builds in the same checkout:
the binary and packaging paths are shared. Forced termination or host loss cannot
run shell cleanup; always require a successful fresh invocation, not merely the
presence of a file, as build evidence.
Version-check failures occur before output paths are selected and leave existing
files untouched; those files are not evidence of a successful invocation.

### Builder/link evidence, not distribution approval

`scripts/build-provenance.py` performs the one final Cargo link used by the helper.
It first runs target/release-scoped `cargo clean --package herdr-crossroads`,
leaving third-party dependency caches intact. Otherwise Cargo can replay cached
link diagnostics without recreating a removed map; a fresh final link is required.
It requests a GNU/lld map with `-Wl,-Map=...`, file trace with `-Wl,--trace`, and
linker version with `-Wl,-v`. The JSON binds the exact executable SHA256 to the
normalized map SHA256 and retains the build argv, selected build environment,
Rust/Cargo/GCC versions, and raw link output/diagnostics (including rustc's printed
link command). It records paths, resolved paths, hashes and available installed
binary/source package versions for recognized whole-line absolute file traces
and a conservative set of GCC-resolved startup/nonshared/runtime candidates.
Unavailable temporary objects and unowned files are explicit; no hashes or package
versions are inferred. Trace inputs are files opened by the linker, not proof of
incorporated code; GCC defaults may differ from the actual driver's flags.
Manually reconcile the raw map and command with these records. There is no
universal map parser or automatic attribution verdict.

Automatic trace indexing accepts unquoted absolute paths without whitespace before
an optional archive-member suffix. Verbose linker commands and ambiguous paths
containing whitespace are not automatic file records. Normalized raw link
output/diagnostics and the map remain available for manual reconciliation.
Only a missing input (`FileNotFoundError`) is marked unavailable; nonregular inputs
and other stat/read errors fail collection. Symlinks to regular files retain both
the original and resolved paths. Private prefixes are normalized in failure
diagnostics too.

Repository, Cargo, Rustup and home prefixes are replaced with neutral placeholders
in both sidecars; system paths such as `/usr/...` remain intact. Map hashes cover
these normalized bytes. Sidecars are outside the strict four-member tar, not
installer inputs. A missing/empty map or collection error blocks packaging; failed
gates remove current-version sidecars alongside the archive/checksum. Do not reuse
historical evidence for a new binary or builder.

The canonical notices conservatively include both reviewed builders in stable
order, not just the current host. The Ubuntu profile was reconciled against exact
official package payloads matching the actual PR run's candidate hashes; it is not
a relabeled local inventory. [Source/package provenance and limits](releases.md#ubuntu-2404-system-profile)
include the exact patched glibc headers. The mandatory pre-build system check has
passed locally and in the [fresh Ubuntu CI run](#current-001-public-pipeline-evidence).
Neither a profile match nor collection proves linked contributions or distribution
readiness: retain the manual matching raw-map/ELF release gate.

On 2026-10-16 the offline local dual-profile build passed the mandatory notice gate,
a fresh final link, all four full PTYs and packaging. Separate archive checking
confirmed four regular members, preserved executable mode, both checksums, exact
canonical notice bytes, binary/map hash binding and normalized private prefixes.
The binary remained byte-identical to the earlier local executable. Both isolated
real-Herdr 0.9.5 harnesses subsequently passed against this repackaged asset; see
[dual-profile native acceptance](installation.md#dual-profile-local-acceptance).
The later [0.0.1 CI run](#current-001-public-pipeline-evidence) passed the new notice gate.

Local collector verification on 2026-10-15 used the offline helper invocation
above: a fresh link, all four full PTYs, packaging, binary/map SHA256 binding,
four-member archive, notice byte identity, and private-prefix removal passed.
Its actual startup-object records identify local glibc `2.43-2ubuntu2.4`;
this is not evidence of the Ubuntu 24.04 runner's glibc version.

Verify the archive from the repository root:

```sh
(cd target/builds && sha256sum -c herdr-crossroads-0.2.0-x86_64-unknown-linux-gnu.tar.gz.sha256)
```

After extracting the tar into a separate empty directory, run
`sha256sum -c herdr-crossroads.sha256` there. Neither packaging nor extraction
installs the plugin. The manifest launches
`./target/release/herdr-crossroads` from the linked checkout; the explicit-target
binary is not automatically placed at that path by the build helper. The separate
[binary installer](installation.md) validates the tar and atomically installs
that binary at the manifest path without Rust/Cargo. An explicit trusted local
asset directory selects offline installation; otherwise it uses the prepared
version-pinned GitHub Release route. The regular `0.1.0` release and both test
prereleases are published; their separately scoped
[public installation evidence](installation.md#regular-010-public-installation)
does not establish universal runtime compatibility.
The [source build-then-link route](../README.md#installation)
also remains available. The public source is
`eisenhauer-dev/herdr-crossroads`; public repository access was verified on
2026-10-15. The published 0.0.1 and 0.0.2 prereleases have separate reviewed assets.

## Historical build and pipeline evidence

The dated records below retain their original artifact-specific scope and
headings. Use [Build and verify locally](#build-and-verify-locally) for the current
prerequisites and commands, not these historical receipts.

## Regular 0.1.0 public pipeline evidence

- [Run 38903556630](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38903556630),
  artifact `12046949385`, passed formatting, Clippy, 258 Rust tests, Python helpers,
  the exact-builder notice gate, the build, all four full PTYs and packaging on its first attempt.
- Reviewed source `490b0d74dc866b6ed83786f7aaa5657c35db5c51` matches the PR checkout's
  tree `a18018e4a56c60d7ba814e6e44a6de2d0a595db2`. PR #10 later merged as
  `9ed72efed62672a1cee087bb9d3e353cce52276b` with that same tree.
- Independent downloaded-artifact verification checked ZIP SHA256
  `6b4fdfa65b115b5ac4ef4faf42b1420753370b566a8bd757eb72e2f241f58724`, exactly four
  assets, four regular root-owned tar members with modes `0755`/`0644`, canonical
  notices, both checksums and binary/map binding.
- All 13 reviewed Ubuntu profile candidates and 14 collected candidates matched;
  the bound map had 14 covered startup rows. ELF inspection confirmed PIE, dynamic
  libc/libgcc/loader dependencies, no static system archive contributions,
  RPATH/RUNPATH, uncovered startup symbols or personal home prefix.
- All four primary-owned local PTYs also passed against the downloaded CI binary;
  synthetic search updates measured 1.48/1.91/1.48 ms, not a live Herdr benchmark.
- Published archive SHA256:
  `d55b5bc611f74a7f1a61de789428e01fcf9b414640153c796f10a6a8a049304e`.
  Binary SHA256: `63857e066f8e4c7821114107292e5e973aaab0b12144ab13b1a63e59611bace1`.
  All four public downloads matched the approved CI hashes exactly.
- The binary hash matches historical CI evidence because Rust source/version,
  toolchain and candidate-profile inputs match. This was a fresh CI build and
  canonical-notice packaging run, not historical asset reuse.
- Receipt: `target/public-pipeline/38903556630/verification.json`. Its earlier
  publication-pending limit describes that checkpoint; subsequent publication and
  [public installation](installation.md#regular-010-public-installation) passed separately.
  Minimum-runtime, source-signature and historical behavior-RED limitations remain.

## Current 0.1.0 local preparation evidence

- Formatting, Clippy, 258 Rust tests, the matching-builder notice gate, the fresh
  explicit-target build, all four full release PTYs and packaging passed. The
  synthetic search updates measured 1.75/1.79/1.76 ms, not a live Herdr benchmark.
- Artifact verification confirmed exactly four regular root-owned tar members,
  preserved modes `0775` (binary) and `0664` (other members), both checksums,
  canonical notices and binary/map binding.
- All 13 reviewed local glibc 2.43 profile candidates matched; 14 candidates were
  collected, including unused `crt1.o`. The bound map had 14 covered startup rows.
  The PIE dynamically needs libc, libgcc and the loader, with no RPATH/RUNPATH,
  static system archive contributions or uncovered startup-unit contributions.
- Receipt: `target/regular-release/local-verification.json` (ignored local evidence),
  separate from all historical receipts. The fresh archive SHA256 is
  `11651272b2c1f88bbe96af971b1d373eedd7c8bcb960d9c8c237321f51f8258a`;
  the normalized map SHA256 is
  `0aec86ec32b01bf48e1355062f6cea9a8902b2fc7bc8e1b4d499d321fcc6ebe8`.
- The binary SHA256 `146d556e5afee59c61b0664b85457b16b31764d0662062ce553edc4374248c1c`
  matches historical local `0.1.0`: the approved toolchain, dependencies and Rust
  source/version are unchanged. This run generated fresh gate, map and archive
  evidence; it did not rename or reuse historical assets.
- **The local binary contains personal build paths and is not a publication
  candidate.** The regular release instead used the reviewed clean CI assets above.
  This local result alone establishes no
  minimum runtime, public installation or publication; the documented source-signature
  and historical policy behavior-RED limitations remain.

## Local 0.0.2 preflight evidence

The published test version `0.0.2` has separate evidence. On 2026-10-16 its fresh local build
passed formatting, Clippy, 258 Rust tests, the matching-builder notice gate, all
four full release PTYs and packaging. Separate artifact verification confirmed
exactly four regular root-owned tar members, both checksums, canonical notices,
and binary/map binding. Preserved modes are binary `0775`, other members `0664`;
the installer normalizes installed modes separately.

| Local 0.0.2 evidence | SHA256 |
| --- | --- |
| Archive | `ab78229b1d00c418c470dc8b8cc1a78978f8bd8ef550732fe90d528aa9b18bf0` |
| Binary | `dfcb503acc84a6d25b95a0050e282e3cbd70c99df77294abf37726c557749d5d` |
| Normalized map | `ec19a021ae9ed3a59e9f74a19c0131070ebdebec2d55921df2a4b61d163d4b71` |

The reviewed local glibc 2.43 profile's 13 candidates matched; collection also
included unused `crt1.o`, absent from the map. All 14 bound startup rows were
covered glibc/GCC contributions, with no static system archive contributions,
RPATH/RUNPATH or uncovered init contributions. The ELF dynamically needs libc,
libgcc and the loader. Receipt: `target/public-cycle/0.0.2-verification.json`.
This is local artifact evidence, not an Ubuntu/minimum-runtime claim or public
acceptance; the glibc DSC-signature limitation remains. This local binary contains
personal build paths and was **not published**; only the clean reviewed CI assets
below were published. The `0.0.1` evidence remains bound to its original assets.

## Current 0.0.2 public pipeline evidence

On 2026-10-16 public PR [run 38877336960, attempt 2](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38877336960/attempts/2)
passed the Linux gates, exact-builder notice check, all four full PTYs, packaging
and upload. Attempt 1 failed in an unchanged transport fixture with
`ExecutableFileBusy` rather than expected `Other`; the unchanged rerun passed
without assertion or gate changes. The root cause remains uncertain.

Artifact `12035673226` records reviewed source
`c24df0876dae2a219bb833695829f7dc99433ae2`. The actual PR checkout was
`6919b48116fb27bc2f130d7290c8d11a48d3552a`; its tree
`10096479692e19c0ab4aa8d41117f45954c04629` equals the reviewed source tree.

| Downloaded 0.0.2 evidence | SHA256 |
| --- | --- |
| Actions ZIP | `6db6f71c14538fb492a29127651fc9a1da8d0df19897b0500ef52caad395a3e8` |
| Archive | `12b0aec9b8370b5cd30c70733d788034fb6f7c38e2b77f026f6b81d4bdec2a16` |
| Binary | `588f25fef39752bfaab8218768e8587afcc067a71fca2dfff5d6af27b103f015` |
| Normalized map | `6fb4a859e747b13cba3123e20dd74e06e8a07e0ef4e4c9a556376694cdff4b42` |
| Provenance JSON | `0891df959f83f127ddfd4b7be6116ab5cf68d97dc704b469c9d7b7f0925bf4de` |
| Archive checksum file | `2a7484ab7dae100c17b2be19e51199b06413019152054cea72cf303de25476a5` |

Independent verification checked the ZIP digest and exact four assets, both
checksums, canonical notices, and binary/map binding. The tar has exactly four
regular root-owned members, binary mode `0755` and other modes `0644`. All 13
profile candidates and all 14 collected candidates matched the fully reviewed
0.0.1 package/source identities and bytes. Matching ELF/map review found 14 covered
startup rows, dynamic libc/libgcc/loader dependencies, no static system archive
contributions, RPATH/RUNPATH or uncovered startup symbols, and no personal home
prefix. All four fresh local PTYs also passed against the downloaded CI binary.
Receipt: `target/public-pipeline/38877336960/verification.json`.

Those exact four assets were published as the retained non-latest
[v0.0.2 prerelease](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.0.2),
whose tag points to the reviewed source above. The [real public lifecycle](installation.md#public-installupdatereturn-cycle)
then passed. This is artifact-specific evidence, not regular-release approval,
a minimum-runtime claim or a universal legal/security guarantee. The glibc
DSC-signature and historical behavior-RED timing limitations remain. The receipt's
publication-pending entry describes its earlier verification checkpoint, not the
subsequent publication/cycle result.

## Local 0.0.1 preflight evidence

The older build and native `0.1.0` results later in this guide are historical,
not verification of `0.0.1`. A fresh local `0.0.1` build on 2026-10-16 passed formatting,
Clippy, 258 Rust tests, the matching-builder notice gate, all four full release
PTYs and packaging. Separate archive/ELF/map checks confirmed four regular members,
numeric root ownership, preserved source modes (binary `0775`, other files `0664`
on this host), both checksums, exact notice bytes and the reviewed local builder's
13 candidate identities. Installed modes are normalized separately by the installer.

| Local 0.0.1 evidence | SHA256 |
| --- | --- |
| Archive | `93a61fa1972774b7f3c92bc5c1f799752efb3b2a510f3661407971438fabd6c1` |
| Binary | `4db948b2abaf09c9d9899274fcba8174116130b53245de5e664e6846fd2bd998` |
| Normalized map | `c2cbea47445841df8ba5c8dd1947b9c0d020ce90eaff7ce004281f1e62b6b70f` |

The bound map again contains only the covered glibc startup sections and GCC CRT
contributions, with no static `libc_nonshared.a`, `libgcc.a` or `libgcc_eh.a` members.
The PIE dynamically needs libc, libgcc and the loader, has no RPATH/RUNPATH, and
lacks `_IO_stdin_used` and `_dl_relocate_static_pie`. This is artifact-specific local
evidence, not public CI, distribution acceptance, or a legal/security guarantee.
The documented source-signature limits remain. Existing historical local artifacts
were not renamed or removed. See [local installation checks](installation.md#local-001-preflight).

## Current 0.0.1 public pipeline evidence

On 2026-10-16 public PR [run 38867597394](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38867597394)
passed formatting, Clippy, 258 Rust tests, Python helper tests (including source
policy), the mandatory exact-builder notice gate, the explicit-target release
build, all four full PTYs, packaging and upload. This is the fresh Ubuntu 24.04
gate result, distinct from the historical `0.1.0` runs below.

Artifact `12032370902`, `herdr-crossroads-0.0.1-x86_64-unknown-linux-gnu`, records
head `1610de9c969a487bb639881cd0c71fe1ee0e3ee2`. The actual PR checkout was
`448362638a37aee7be3711a1ce9d429386c9e40a`, with parents
`5239c17dd0548f55c76d380998e327a6766e86dd` and that reviewed head; GitHub's tree
`b256a58a781fa1455b93f8a47cb23d04357a8520` equals the reviewed head's tree.

| Downloaded 0.0.1 evidence | SHA256 |
| --- | --- |
| Actions ZIP | `74a7c183e15b091e1b204af233874ad53ef1f565ff2c67e1938d487c46491dc5` |
| Archive | `5750e503cfff7447538f92d3b96f4d60f211a1f3788d0420246cd0bc7b76d31e` |
| Binary | `145898e7e925df0576af9b124afd8c76c2241f2d5ad64691164ec2eb3a029b41` |
| Normalized map | `e14f6ad3cca0e8891a8561a738466df1cc6b30c7cde3d66d0ba720c1ddfd90ea` |

Independent primary verification matched the ZIP digest and its exact four files
to the downloaded archive/checksum and JSON/map sidecars. The tar has exactly four
regular members, numeric root ownership, binary mode `0755` and other modes
`0644`; both checksums and both notice files matched the trusted commit.
All 13 GCC candidates matched the reviewed Ubuntu profile; all 14 collected
candidates, including `crt1.o`, matched the prior primary package proof's bytes
and binary/source package identities. The checked-in profile, startup-source
evidence and canonical notices were hash-identical to that proof.

Fresh checks of this bound ELF/map found PIE, no RPATH/RUNPATH, no
`_IO_stdin_used` or `_dl_relocate_static_pie`, only 14 covered glibc/GCC startup
rows, and no static `libc_nonshared.a`, `libgcc.a` or `libgcc_eh.a` members.
The retained local receipt is
`target/public-pipeline/38867597394/verification.json` (ignored build evidence,
not a published asset). The [local native trial](installation.md#pipeline-asset-local-trial)
also passed with these exact downloaded bytes. This is artifact-specific evidence,
not distribution approval, public Release installation, a minimum-runtime claim,
or a universal legal/security guarantee. The
[unverified glibc DSC-signature boundary](releases.md#ubuntu-2404-system-profile)
is unchanged.

Those exact four assets were subsequently published as the non-latest
[v0.0.1 prerelease](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.0.1)
from reviewed source `1610de9c969a487bb639881cd0c71fe1ee0e3ee2`.
The isolated Herdr 0.9.5 public CLI-install and pinned native single-version tests
passed on 2026-10-16. All four public asset downloads matched the approved hashes;
[main CI run 38871442877](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38871442877)
also passed. The later [two-version cycle](installation.md#public-installupdatereturn-cycle)
also passed. The single-version local receipt is
`target/public-cycle/0.0.1-verification.json`.

## Runtime and verification limits

Local verification on 2026-10-13 used Linux x86_64, Rust/Cargo 1.99.0, and glibc
2.43. The exact target binary passed all four complete isolated PTYs; tar contents,
executable mode, and both checksums were checked locally. This is fake-host test
evidence; that build run did not test native Herdr installation.
Those historical artifacts predate the four-member notice contract. A fresh
2026-10-15 build passed all four full PTYs with its then-current local-only notices; archive
membership, preserved executable mode, notice byte identity and both checksums
were verified. Its binary matches the [checked local link map](releases.md#installed-system-provenance-and-verified-startup-exceptions).

Separately, on 2026-10-14 the unchanged local 0.1.0 artifact passed the isolated
[native Herdr 0.9.3 managed lifecycle test](installation.md#native-distribution-acceptance),
including search/focus, replacement with a test-only trailer fixture, return to
the original version, and uninstall. No Rust/Cargo ran in that installation
sandbox. This is local runtime evidence, not public distribution or compatibility
with a genuinely new runtime version.

On 2026-10-15 the current four-member artifact also passed all five isolated
real-Herdr 0.9.5 CLI install cases and the real-server/client managed lifecycle:
installation, search/focus, test-fixture replacement, return, uninstall and
shutdown. See [current hashes and scope](installation.md#native-distribution-acceptance).
This does not test Herdr's separate update command or delayed generation reclamation.
Those recorded native results precede dual-profile notice regeneration; the
[separate repeated acceptance](installation.md#dual-profile-local-acceptance)
covers the newly repackaged archive.

`readelf` inspection showed ELF64 AMD x86-64, the interpreter
`/lib64/ld-linux-x86-64.so.2`, and dynamic dependencies `libc.so.6`, `libgcc_s.so.1`,
and `ld-linux-x86-64.so.2`. This is not a static or musl binary. Runtime also needs
`stty` and a running Herdr session (manifest minimum 0.9.1); see
[requirements and compatibility](../README.md#requirements-and-compatibility).
ELF symbol versions do **not** establish a tested minimum glibc version or
distribution compatibility. No older runtime was tested, and no minimum glibc or
distribution support guarantee is made.

The public-only **Linux gates** CI job builds the same target on
Ubuntu 24.04, then uploads the tested tar, its checksum and the two builder-evidence
sidecars using commit-pinned `actions/upload-artifact` v7.0.2 with seven-day
retention and an error on missing files. The tar preserves executable mode inside
the Actions artifact. Private-repository events still skip the entire job.
`workflow_dispatch` prepares the same gates on a selected existing ref. Only after
successful packaging does that workflow step set the versioned artifact name in
`GITHUB_OUTPUT`; the upload uses exactly those four explicit filenames, not a wildcard
that could select a stale prior version. The public
[main run on 2026-10-15](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38775076190)
passed the gates, explicit-target build, all four full isolated PTYs, packaging,
and upload on Ubuntu 24.04. This is CI evidence, not a live Herdr integration test
on that runner or a dry-run release publication. Independent downloaded-artifact
verification is recorded below; neither this run nor the local glibc 2.43
results establishes a minimum glibc version or older-runtime compatibility.

## Downloaded public pipeline verification

On 2026-10-15, artifact `11993142568` from the public main run above was
downloaded and checked independently. Its source commit was
`5239c17dd0548f55c76d380998e327a6766e86dd`, version `0.1.0`, target
`x86_64-unknown-linux-gnu`.

The historical `0.1.0` CI artifacts were deleted remotely on 2026-10-16 with user
approval. Retained local copies and these verification records are historical
evidence, not currently downloadable CI assets or renamed `0.0.1` assets.

- Archive SHA256: `153b261e467d98e364968ca964e7864e77efb4a89ae8f103b150deaf6baffddf`.
- Binary SHA256: `63857e066f8e4c7821114107292e5e973aaab0b12144ab13b1a63e59611bace1`.
- Both checksum records matched. The tar contained exactly the four regular
  release members, numeric root ownership, binary mode `0755`, and other file
  modes `0644`; both notice files matched the source checkout byte-for-byte.
- ELF inspection confirmed Linux x86-64, interpreter
  `/lib64/ld-linux-x86-64.so.2`, and dynamic libc/libgcc dependencies.
- All four full isolated PTYs passed against the downloaded executable on the
  local Linux host. The 1,000-pane search measured 0.86/0.92/1.76 ms per update.

This proves pipeline delivery and local execution of those exact bytes, not a
published release or live public Herdr installation. Matching the checked-in
notices does not independently establish the CI builder's system/startup-object
provenance or final-link evidence; those remain to be checked before distribution.
