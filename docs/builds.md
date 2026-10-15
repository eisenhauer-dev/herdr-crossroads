# Tested Linux build artifacts

The local build helper and prepared CI job support exactly
`x86_64-unknown-linux-gnu`. Artifacts are version-named for local release
preparation, not published releases. The build helper makes no tags, publication,
plugin activation, or installation changes. See [release preparation](releases.md)
for the version contract and separate approval/verification boundaries.

## Build and verify locally

Run from a trusted checkout on native Linux x86_64. Have Rust/Cargo, a system
linker, the already-installed `x86_64-unknown-linux-gnu` Rust target, Python 3.11+,
`stty`, a POSIX shell, GNU tar, and `sha256sum` available. The PTYs require an
existing owned writable `/tmp/opencode` directory; they use fake Herdr executables
and sockets, not a live Herdr session.

From the repository root:

```sh
python3 scripts/check-version.py
python3 tests/test_version.py
python3 tests/test_build_linux.py
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
exact `rustc --version`. Dependency, feature, or toolchain changes require notice
review and regeneration; changing only the project's version does not. Missing
license files or stale inventory stop before Cargo and remove current-version
artifact outputs. See [notice maintenance and final link-map verification](releases.md#license-inventory-and-verification).

The helper rejects other native operating systems/architectures, fixes Cargo's
output directory to this checkout's `target/`, and builds with
`cargo build --release --locked --target x86_64-unknown-linux-gnu`. It removes the
previous artifact files for the agreed version and target binary before building, then runs
`pty_latency.py`, `pty_render.py`, `pty_keymaps.py`, and `pty_hints.py` in that
order against exactly `target/x86_64-unknown-linux-gnu/release/herdr-crossroads`.
The complete keymap gate is used, never `--input-protocol`.

Only after all four full PTYs pass does it produce these ignored outputs:

```text
target/builds/herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz
target/builds/herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
```

These names use the current agreed version `0.1.0`, not a fixed literal in the
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
archive checksum, and temporary packaging files; handled INT/TERM interruptions
do likewise. A failed invocation never advertises success or reuses an old binary
as its new build. Do not run concurrent helper/Cargo builds in the same checkout:
the binary and packaging paths are shared. Forced termination or host loss cannot
run shell cleanup; always require a successful fresh invocation, not merely the
presence of a file, as build evidence.
Version-check failures occur before output paths are selected and leave existing
files untouched; those files are not evidence of a successful invocation.

Verify the archive from the repository root:

```sh
(cd target/builds && sha256sum -c herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256)
```

After extracting the tar into a separate empty directory, run
`sha256sum -c herdr-crossroads.sha256` there. Neither packaging nor extraction
installs the plugin. The manifest launches
`./target/release/herdr-crossroads` from the linked checkout; the explicit-target
binary is not automatically placed at that path by the build helper. The separate
[local-asset installer](installation.md) validates the tar and atomically installs
that binary at the manifest path without Rust/Cargo; it requires an explicit
trusted asset directory. The [source build-then-link route](../README.md#installation)
also remains available. The selected future source is
`SimonEisenhauer/herdr-crossroads`; no public repository/tag/asset availability
or download endpoint is established.

## Runtime and verification limits

Local verification on 2026-10-13 used Linux x86_64, Rust/Cargo 1.99.0, and glibc
2.43. The exact target binary passed all four complete isolated PTYs; tar contents,
executable mode, and both checksums were checked locally. This is fake-host test
evidence; that build run did not test native Herdr installation.
Those historical artifacts predate the four-member notice contract. A fresh
2026-10-15 build passed all four full PTYs with the current notices; archive
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

`readelf` inspection showed ELF64 AMD x86-64, the interpreter
`/lib64/ld-linux-x86-64.so.2`, and dynamic dependencies `libc.so.6`, `libgcc_s.so.1`,
and `ld-linux-x86-64.so.2`. This is not a static or musl binary. Runtime also needs
`stty` and a running Herdr session (manifest minimum 0.9.1); see
[requirements and compatibility](../README.md#requirements-and-compatibility).
ELF symbol versions do **not** establish a tested minimum glibc version or
distribution compatibility. No older runtime was tested, and no minimum glibc or
distribution support guarantee is made.

The public-only **Linux gates** CI job is prepared to build the same target on
Ubuntu 24.04 (intended glibc 2.39 runner), then upload only the tested tar and its
checksum using commit-pinned `actions/upload-artifact` v7.0.2 with seven-day
retention and an error on missing files. The tar preserves executable mode inside
the Actions artifact. Private-repository events still skip the entire job.
`workflow_dispatch` prepares the same gates on a selected existing ref. Only after
successful packaging does that workflow step set the versioned artifact name in
`GITHUB_OUTPUT`; the upload uses exactly that archive and checksum, not a wildcard
that could select a stale prior version. This is prepared CI release-preflight
configuration, not an executed CI run or a dry-run release publication.
No Actions execution, upload, downloaded-artifact verification, or Ubuntu 24.04
runtime result is claimed yet. Public CI/artifact evidence remains outstanding;
local glibc 2.43 results do not verify the intended runner or older runtimes.
