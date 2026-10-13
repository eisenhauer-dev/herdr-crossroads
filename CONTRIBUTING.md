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

## Optional local pre-commit and pre-push checks

The repository-owned `.githooks/pre-commit` runs `cargo fmt --check`, then
`cargo clippy --locked --all-targets -- -D warnings` from the worktree root.
Formatting failure stops before Clippy; either failure, including missing Cargo
or tooling, blocks the commit. It does not autofix files or stage changes.
The repository-owned `.githooks/pre-push` runs `cargo test --locked` from the
worktree root; test failure or missing Cargo blocks the push. It does not modify
the index or working files (Cargo may create build outputs).
These checks use the **current working tree**, not a reconstructed staged
snapshot: unstaged changes can cause failures or hide problems in staged content.
Pre-push does not automatically check every pushed commit: the current working
tree may differ from those commits, so passing tests does not prove their contents.
Cargo may fetch uncached dependencies as with the manual commands above.

Activation is explicit and optional; cloning does not activate either hook. From
the repository root, first inspect and record any prior setting and its scope:

```sh
git config --show-origin --get-all core.hooksPath
```

No output and exit status 1 means no setting. The following setup refuses any
existing effective `core.hooksPath`, including inherited settings, instead of
silently replacing it. If one exists, leave it unchanged and run the checks
manually; integrating with another hook setup is not covered here.

```sh
(
    if git config --get-all core.hooksPath; then
        printf '%s\n' 'Existing hooksPath: leave it unchanged; do not run setup.' >&2
        exit 1
    fi
    git config --local core.hooksPath .githooks
)
```

This changes only this repository's local Git configuration, never global Git
configuration. Local configuration is shared by its linked worktrees, so this
is not activation for just one worktree; the setting activates both hooks, and
each worktree needs executable `.githooks/pre-commit` and `.githooks/pre-push`
from this change. Git ignores non-executable hooks. Existing
`.git/hooks` files are left untouched but are not used while this setting is active.

To remove this opt-in, first confirm the local setting is still the `.githooks`
value installed above (`git config --local --get-all core.hooksPath`). If it has
changed since setup, do not unset someone else's replacement. Otherwise run:

```sh
git config --local --unset core.hooksPath
```

This restores the prior unset local setting and normal hook resolution, including
any existing `.git/hooks` hooks. Inherited configuration remains untouched; any
setting added elsewhere since activation will apply normally. No hook files need
to be deleted or copied into `.git/hooks`.

Run the isolated, standard-library regression tests with:

```sh
python3 tests/test_git_hooks.py
```

They exercise real Git commits and local pushes with fake Cargo in temporary
repositories under `/tmp/opencode`, not this checkout's configuration, a network
remote, or a real Herdr session.
The hooks are early feedback only and can be bypassed with `git commit --no-verify`
or `git push --no-verify`: CI remains mandatory. Pre-push runs Cargo tests, not the
complete optimized-release PTY gates below; those remain separate requirements.

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
