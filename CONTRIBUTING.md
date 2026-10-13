# Contributing

## Developer setup

Use Linux with Rust/Cargo, a system linker, `stty`, a POSIX shell, and Python
3.11+ for the standard-library-only PTY harnesses. The tests require an existing,
writable `/tmp/opencode` directory for isolated temporary files and Unix sockets;
create it only if you own it and have permission. No real Herdr process, plugin
link, user configuration change, or host keybinding is needed for automated tests.

The documentation verification used Rust/Cargo 1.93.1 and Python 3.14.4 on Linux
x86_64. `Cargo.toml` specifies edition 2021 but no minimum Rust version; do not
interpret that verification version as an established MSRV. Dependencies are
pinned in the manifest and `Cargo.lock`; do not add dependencies merely to write
documentation or run the existing Python harnesses.

The current local lint and test verification uses Rust/Cargo 1.99.0, with Clippy
0.1.99 and rustfmt managed alongside the toolchain by rustup. This is a verified
tooling version, not an MSRV or a toolchain pin.

From the repository root:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

Cargo may fetch dependencies on a fresh machine. Add `--offline` when the pinned
dependencies are already cached and you need a network-free check. Do not use
`cargo install` as plugin setup: the manifest expects the release binary inside
the checkout. See [README installation](README.md#installation) for optional
manual build-then-link testing; those Herdr examples were not executed during
documentation verification.

## Test layers and release gates

- `tests/unit/`: module-level tests, included from `src/lib.rs`; keep test code
  and fixtures under `tests/`, not alongside production modules.
- `tests/a10c.rs`: startup/configuration integration checks using fake Herdr.
- `tests/startup.rs`: snapshot, focus, live-status, reconnection, and process
  behavior checks. On Linux it invokes the latency/render PTYs and the
  **input-protocol subset** of the keymap PTY.
- `tests/fixtures/topology.json`: deterministic workspace/tab/pane/agent snapshot.
- `tests/pty_*.py`: isolated raw-terminal regressions with fake CLI/socket data.
  The latency harness measures per-character search redraws for 1,000 panes,
  checks match counts and underlining, and requires ≤100 ms per measured update.

Before release, run all four complete harnesses against a freshly built release
binary, in addition to formatting, Clippy, and Cargo tests:

```sh
python3 tests/pty_latency.py target/release/herdr-crossroads
python3 tests/pty_render.py target/release/herdr-crossroads
python3 tests/pty_keymaps.py target/release/herdr-crossroads
python3 tests/pty_hints.py target/release/herdr-crossroads
```

Do not substitute `--input-protocol` for the complete keymap gate, or assume
`cargo test` runs the hint gate. [Keymaps local verification](docs/keymaps.md#local-verification)
defines the complete gate requirements. Passing fake-host tests does not prove
live installation, host popup routing, SSH, every keyboard protocol, or cross-OS
compatibility. Any manual host test should state the Herdr version, terminal,
platform, setup changes, results, and cleanup performed.

Coverage has no numeric minimum initially. Record the baseline, explain decreases
in review, and meaningfully test relevant changed paths and error paths rather
than chasing a percentage. See [coverage](docs/coverage.md) for the source-only
baseline and reproducible native LLVM measurement. Coverage supplements, never
replaces, the hard formatting, Clippy, Cargo-test, and complete optimized-release
PTY gates above.

## Making a change

1. Read the affected module, tests, and [architecture](docs/architecture.md).
   Keep `main.rs` thin and respect existing module responsibilities.
2. Keep the change narrowly scoped. For behavior changes, first demonstrate a
   failing regression, implement the smallest fix, then rerun the focused test.
   Exercise relevant boundaries, errors, and preserved behavior; never weaken a
   failing assertion just to get green. Docs-only changes need validation, not
   invented failing tests.
3. Reconcile user-facing docs with final implementation and tests. Put detailed
   behavior in `docs/`, not a per-function API catalog. Avoid speculative options,
   compatibility promises, or unmeasured performance claims.
4. Run formatting, Clippy with warnings denied, Cargo tests, and applicable PTY
   gates. Review your diff for unrelated changes and unintended generated/config
   files.
5. In a contribution, explain the problem, scope, source evidence, test commands
   and results, limitations, and any unexecuted examples. Report toolchain and
   platform details for input/rendering regressions.

For bug reports, include expected/observed behavior, minimal reproduction steps,
Herdr/plugin versions, terminal and content dimensions, OS/architecture, and
whether the issue occurs with default configuration. Share only a minimal,
sanitized configuration or fixture; do not post sockets, private workspace names,
credentials, or full personal config/logs.

## Safety and ownership

Crossroads stays navigation-only. It must not install/update during opening or
search, rewrite native prefix `g`/`t`, change other plugins, or alter Herdr config.
Tests should use isolated fake executables and sockets, never a live session by
accident. Keep plugin configuration precedence, strict validation, exact pane-ID
focus, terminal-control sanitization, and terminal restoration covered when
touching those areas. Theme persistence is plugin-owned and explicitly initiated
by the user; see [themes](docs/themes.md) for its write/delete contract.

The project uses the [MIT license](LICENSE).
