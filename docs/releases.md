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
and `herdr-crossroads.sha256`. Ownership is normalized to numeric root. Both
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
matching the local preflight list above. Installer unit tests use synthetic assets
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
fixture; it does not prove new-runtime upgrade compatibility. The local-only
installer is separate from public distribution;
no public installer readiness or actual GitHub release automation is claimed.
The opt-in real-Herdr sandbox test is main-owned, not a routine release command.
The selected future source is `SimonEisenhauer/herdr-crossroads`; public
repository/tag/asset availability, run/upload/download evidence and final release
approval remain open.
