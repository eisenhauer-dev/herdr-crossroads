# Tested Linux build artifacts

The local build helper and CI job support exactly
`x86_64-unknown-linux-gnu`. Artifacts are version-named for local release
preparation; building does not publish releases. The build helper makes no tags, publication,
plugin activation, or installation changes. See [release preparation](releases.md)
for the version contract and separate approval/verification boundaries.

The current preparation version is `0.0.2`. On 2026-10-16 its fresh local build
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
acceptance; the glibc DSC-signature limitation remains. Version 0.0.2 publication
and the real public update/return cycle remain pending. The `0.0.1` evidence below
remains bound to its original assets.

The older build and native results below are historical `0.1.0` evidence, not
verification of `0.0.1`. A fresh local `0.0.1` build on 2026-10-16 passed formatting,
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
also passed. The two-version cycle remains unrun. The local receipt is
`target/public-cycle/0.0.1-verification.json`.

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
target/builds/herdr-crossroads-0.0.2-x86_64-unknown-linux-gnu.tar.gz
target/builds/herdr-crossroads-0.0.2-x86_64-unknown-linux-gnu.tar.gz.sha256
target/builds/herdr-crossroads-0.0.2-x86_64-unknown-linux-gnu.provenance.json
target/builds/herdr-crossroads-0.0.2-x86_64-unknown-linux-gnu.link.map
```

These names use the current test-preparation version `0.0.2`, not a fixed literal in the
helper. Other versions' outputs are left untouched and never selected as this
invocation's artifacts. The helper does not write `GITHUB_OUTPUT` or other workflow
metadata into caller-selected files.

The tar contains exactly four regular files: `herdr-crossroads` (with its executable
mode preserved), `herdr-crossroads.sha256` (the binary's SHA256), `LICENSE`
(the project's full MIT terms), and `THIRD-PARTY-NOTICES.html` (full dependency
license/attribution texts, Rust standard-library notices, and installed system
startup/runtime copyright/full-license records and exact glibc startup linking
exception headers). Keep the notices
with redistributed binaries. Archive ownership is normalized
to numeric root, rather than recording the builder's username. The adjacent file
checksums the tar. Checksum entries contain filenames, not checkout paths.
Checksums detect corruption, not authenticity or publisher identity.

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
(cd target/builds && sha256sum -c herdr-crossroads-0.0.2-x86_64-unknown-linux-gnu.tar.gz.sha256)
```

After extracting the tar into a separate empty directory, run
`sha256sum -c herdr-crossroads.sha256` there. Neither packaging nor extraction
installs the plugin. The manifest launches
`./target/release/herdr-crossroads` from the linked checkout; the explicit-target
binary is not automatically placed at that path by the build helper. The separate
[binary installer](installation.md) validates the tar and atomically installs
that binary at the manifest path without Rust/Cargo. An explicit trusted local
asset directory selects offline installation; otherwise it uses the prepared
version-pinned GitHub Release route; 0.0.2 still awaits publication and acceptance.
The [source build-then-link route](../README.md#installation)
also remains available. The public source is
`eisenhauer-dev/herdr-crossroads`; public repository access was verified on
2026-10-15. The published 0.0.1 prerelease is separate from current 0.0.2 preparation.

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
