# Contributing

## Public contribution workflow

Repository write access is limited to the owner and collaborators explicitly
designated by the owner; currently only the owner is a collaborator. Other
contributors work in forks and open pull requests targeting `develop`, not `main`.
Anyone may clone the public repository. The contribution flow is:

- Fork or internal branch → `develop`: contributions are integrated and tested here.
- Internal branch → `main`: only branches from `eisenhauer-dev/herdr-crossroads`
  are accepted. This is not restricted to the branch named `develop`.
- Fork → `main`: not accepted; target `develop` instead. Fork pull requests to
  `develop` remain welcome.

The owner controls which changes and commits move from development into a
release, and when. This routing policy does not automatically merge, close, tag,
or release anything. The source-repository check in
`.github/workflows/pr-source-policy.yml` is active and required on `main`.
It rejects fork-to-`main` metadata without closing the PR or preventing its opening;
fork-to-`develop` contributions remain allowed.

CI runs for pull requests targeting both `develop` and `main`, including fork
pull requests; the `pull_request` trigger has no target-branch filter. Push-triggered
CI remains limited to `main`. Running CI on `develop` does not itself establish
a required-check branch-protection rule there.

Updates to `main` must use
pull requests even for the owner, with administrator enforcement and no bypass;
force pushes and branch deletion are disabled. No independent approval is
required solely to let the owner merge their own pull request. The `Linux gates`
and `PR source policy` checks from the GitHub Actions app (ID `15368`) are mandatory
for all pull requests targeting `main`, including the owner's own branches.
The latter enforces the source-repository metadata policy above. Branches must
be up to date with `main` before merging. Pending, missing or failed required
checks block merging, including for administrators. GitHub also treats `skipped`
and `neutral` check conclusions as passing; do not weaken the public CI job with
conditions that skip its gates. See [required status checks](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches#require-status-checks-before-merging).

The repository became public on 2026-10-15. Branch protection was enabled
immediately afterwards and read back: pull requests are required with zero
mandatory approvals, administrator enforcement, no bypass actors, and no force
pushes or deletion of `main`. No paid service was enabled. The owner retains
administrative power to change settings. No pending invitations or deploy keys
were present; Actions defaults to read-only access and cannot approve pull
requests. Installed GitHub App access still requires owner verification because
the current token cannot list installations. Continue auditing collaborators,
app access, deploy keys and automation permissions rather than assuming branch
protection alone establishes who can write.
See GitHub's [personal repository permissions](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/repository-access-and-collaboration/permission-levels-for-a-personal-account-repository)
and [protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches).

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
0.1.99 and rustfmt managed alongside the toolchain by rustup. CI explicitly selects
1.99.0; this verified tooling version is not an MSRV or a local toolchain pin.

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
or `git push --no-verify`: they do not replace CI. Pre-push runs Cargo tests, not the
complete optimized-release PTY gates below; those remain separate requirements.

## Test layers and release gates

- `tests/unit/`: module-level tests, included from `src/lib.rs`; keep test code
  and fixtures under `tests/`, not alongside production modules.
- `tests/configuration.rs`: startup/configuration integration checks using fake Herdr.
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

For the single supported native build-artifact target, use
`sh scripts/build-linux.sh`. It performs the locked explicit-target release build,
runs all four full PTYs against that exact binary, and packages a tar plus SHA256
checksums only after success. See [tested Linux build artifacts](docs/builds.md)
for prerequisites, versioned output paths, failure cleanup, and compatibility limits.
Packaging additionally requires one of the two exact reviewed system builders
under `LICENSES/` (local glibc 2.43 or actual Ubuntu 24.04 CI): object hashes,
package/source versions and installed notice/license bytes are mandatory build
inputs. Canonical notices conservatively include both profiles. Unknown or changed
builders fail rather than generating host-specific distribution notices; see
[notice maintenance and source provenance](docs/releases.md#license-inventory-and-verification).
See [local release preparation](docs/releases.md) for matching Cargo/plugin versions,
the complete gate sequence, asset review, and separate remote approvals. This
prepares versioned artifacts, not a published release. The separate
[binary installer](docs/installation.md) uses Python 3.11+ standard library,
without Rust/Cargo. Its default version-pinned Release download route passed the
isolated Herdr 0.9.5 public CLI-install and pinned native tests for published 0.0.1; current 0.0.2
publication and full public-cycle acceptance remain pending. Explicit local assets
stay offline with no network
fallback. Run its isolated tests with
`python3 tests/test_install_binary.py`; they use synthetic ELF headers and never
invoke Herdr. `tests/test_herdr_install.py` is a main-owned, explicit opt-in real
Herdr 0.9.3 sandbox harness, not an ordinary automated/CI gate; see its isolation,
approval requirements and execution status in the installation guide.
`tests/test_distribution.py` separately tests the real native server/client PTY
and managed lifecycle; it passed on Herdr 0.9.3 on 2026-10-14, including replacement
and version return with a test-only fixture, not a newly released runtime.
See [native evidence and limits](docs/installation.md#native-distribution-acceptance).
Only `python3 tests/test_distribution_helpers.py` is a routine/CI gate: its
helper tests do not launch Herdr or any subprocess. Both real-Herdr harnesses
remain explicit opt-in, outside discovery and CI.

Coverage has no numeric minimum initially. Record the baseline, explain decreases
in review, and meaningfully test relevant changed paths and error paths rather
than chasing a percentage. See [coverage](docs/coverage.md) for the source-only
baseline and reproducible native LLVM measurement. Coverage supplements, never
replaces, the hard formatting, Clippy, Cargo-test, and complete optimized-release
PTY gates above.

## Automated checks

`.github/workflows/ci.yml` defines one stable **Linux gates** job for pushes to
`main`, `pull_request` events, and manual `workflow_dispatch` on an existing ref,
on the standard Ubuntu 24.04 x86_64 runner. Pull requests from both owner branches
and forks are checked, including new commits, with no PR target-branch filter.
Pushes to `develop` or feature branches and tag pushes do not trigger CI, avoiding
duplicate push and PR runs for development branches. A push to `main` after merging
still runs the checks. Every runner job is guarded by
`github.event.repository.private == false`: private-repository
events skip the job without executing runner steps. When the repository is public
and Actions is enabled, these events automatically run formatting, Clippy with
warnings denied, locked Cargo tests, isolated Git-hook tests, version-checker,
build-helper, installer unit tests, and pure distribution helper tests,
and the explicit `x86_64-unknown-linux-gnu` locked release build with all four
complete PTY harnesses through `scripts/build-linux.sh`.

The job uses rustup already on the runner to select Rust 1.99.0 with rustfmt and
Clippy, verifies Python 3.11+, and creates/checks an owned writable `/tmp/opencode`
directory. It has a 30-minute timeout, read-only contents permission, and a
commit-pinned checkout with credential persistence disabled; no cache or Rust
setup action is used. After successful gates, commit-pinned `actions/upload-artifact`
v7.0.2 uploads only the successful current-run version-named tar, archive
checksum, provenance JSON and link map from `target/builds/`, with seven-day
retention and an error if files are missing. The tar preserves the
binary's executable mode. A successful packaging step supplies the exact asset
name via `GITHUB_OUTPUT`; no wildcard selects previous versions. These are CI
build artifacts, not published release assets. Manual dispatch reuses the same
gates; no tag trigger, publication job, or write permission is configured.

Run the standard-library workflow and provenance regression checks locally with:

```sh
python3 tests/test_ci_workflow.py
python3 tests/test_build_provenance.py
```

These checks protect the main-only push and unfiltered PR/manual trigger contract,
job guard, checkout settings, gate ordering,
helper invocation, and exact versioned artifact upload; they are not a full YAML
parser or GitHub Actions schema validator. `python3 tests/test_build_linux.py` separately
checks full PTY ordering, exact binary selection, checksums/mode, and failure
cleanup with fake tools and real tar/SHA256 in isolated temporary directories. It
also executes the workflow's packaging step in isolation to check that exact-name
metadata is emitted only after success.
`python3 tests/test_version.py` checks agreement, malformed/missing data, and safe
stable version names using isolated manifests and Python 3.11+ `tomllib`.
`python3 tests/test_build_provenance.py` is a required local/CI helper-test gate;
it checks binary/map binding, package/source records, path normalization and
collection failures using fake subprocess results. Evidence limits and the
separate notice-review requirement are documented in [builds](docs/builds.md#builderlink-evidence-not-distribution-approval).
`python3 tests/test_install_binary.py` is also a required helper-test gate locally
and in CI; it uses synthetic assets without invoking Herdr or installing into the
live checkout. `python3 tests/test_distribution_helpers.py` is also required
locally and in CI; it checks fixture assets/versions and current-cell parsing
without invoking Herdr. Both real-Herdr sandbox harnesses remain opt-in and outside CI.
Static checks and successful local gate runs are not proof of a CI execution.
On 2026-10-15, the public [push run](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38764985641)
and [pull-request run](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38765043636)
passed their Linux gates, including binary packaging and artifact upload.
The later [pipeline download verification](docs/builds.md#downloaded-public-pipeline-verification)
checks that historical main artifact separately. Branch protection now requires
**Linux gates** and **PR source policy** from GitHub Actions (app ID `15368`)
and an up-to-date branch before merging into `main`, including for administrators;
see the [contribution workflow](#public-contribution-workflow) for the accepted
check-conclusion limits. Local hooks alone cannot enforce this protection.
No private-repository Actions execution is claimed or required.

### Active PR source merge gate

The separate **PR source policy** job uses `pull_request_target` for `opened`,
`synchronize`, `reopened`, and `edited` events targeting `main` or `develop`.
Unlike the ordinary CI workflow, GitHub takes this workflow from the base
repository's **default branch**, not from the PR's merge commit. The reviewed
workflow reached `main` through [PR #7](https://github.com/eisenhauer-dev/herdr-crossroads/pull/7),
merged at `24e60502de675ca1d6d501ae1cc03421276a03c4` on 2026-10-16.
It leaves `Linux gates` unchanged.

The public-only job uses the standard Ubuntu 24.04 runner, a five-minute timeout,
and `permissions: {}`. It runs only inline Python standard-library code against
`GITHUB_EVENT_PATH`: no checkout, PR code, actions, dependencies, downloads,
caches, secrets, or API access. For `main`, the event repository, PR base repository,
and PR head repository must all have matching positive integer repository IDs.
Missing, null, malformed or mismatched identities fail closed. Branch names and
owner names do not establish repository identity: any internal source branch is
allowed, while a fork branch named `develop` is not. For `develop`, internal and
fork sources are allowed. The check never closes a PR.

On 2026-10-16, `main` protection was configured and read back with strict required
**PR source policy** and **Linux gates** checks, both bound to GitHub Actions app
ID `15368`, with administrator enforcement and existing protections preserved.
The user explicitly waived a real fork-negative test: local event fixtures prove
metadata rejection, but no live fork-PR rejection is claimed. The app binding
is not a claim of absolute spoof resistance: other Actions workflows share the
same app, and administrators retain power to change workflows and protection.
Review workflow changes and keep check names unique.

GitHub's documentation checked on 2026-10-16 says the default public-repository
`pull_request_target` event policy is currently in evaluate mode and will be
enforced for affected repositories on **2026-11-02**. An applicable event policy
explicitly allowing this trusted workflow's event may therefore also require
separate remote approval/configuration. Missing or blocked runs must not be
mistaken for passing enforcement. GitHub may also suppress this event for certain
SHA-like source branch names; such branches are not excluded by the local policy,
but a missing required check can still block merging.
See [securely using pull_request_target](https://docs.github.com/en/actions/reference/security/securely-using-pull_request_target)
and [event behavior and branch filters](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request_target).
These local checks do not themselves verify remote settings or event delivery;
the active configuration above was verified separately by the operator.

Run `python3 tests/test_pr_source_policy.py` locally (also included in CI's helper
checks). It executes the workflow's actual inline policy with isolated event
fixtures under `/tmp/opencode`, covering internal branches, forks, same-name fork
branches, malformed identities, and untrusted strings. Textual checks cover the
event, runner, permissions, and no-checkout/no-secrets contract; these are not a
full YAML/Actions validator or proof of remote execution or branch protection.

Public negative verification on 2026-10-15 used temporary
[PR #6](https://github.com/eisenhauer-dev/herdr-crossroads/pull/6), with `main`
already included in the head branch and no merge conflicts. With `[skip ci]`,
the head had no check runs and GitHub reported its merge state as `BLOCKED`.
An intentional whitespace-only Rust formatting error then failed
[Linux gates](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38777256419)
at `Check formatting`; the merge state remained `BLOCKED`. The required check
was bound to the GitHub Actions app and administrator enforcement remained on.
These were read-only merge-state checks, not attempted merges. The temporary
edits were fully reverted, restoring the file tree of the already-green
[main run](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38775076190),
and the PR was manually closed without merging. No protection rule was changed.
This verification did not create a fork PR or test `skipped`/`neutral` conclusions.

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
Binary archives and installed payloads must retain `LICENSE` and
`THIRD-PARTY-NOTICES.html`. Dependency/toolchain changes require reviewing and
regenerating the exact-version notice inventory; see
[notice maintenance and final link-map verification](docs/releases.md#license-inventory-and-verification).
Run `python3 tests/test_notices.py` on the actual packaging builder with cached
locked dependencies, installed Rust documentation, GCC, `dpkg-query`, and system
copyright/full-license files to reproduce the full notice artifact, including
package versions and startup/runtime object hashes. The build
helper checks manifest/lock/toolchain staleness and matches system candidate hashes,
package/source versions and complete installed notice bytes against a reviewed
builder profile. This does not establish actual final ELF/link contributions.
Exact glibc startup
exception headers and archive provenance are checked in under `LICENSES/` and
embedded in the generated notices. Keep source versions matched, review the final
binary/link map, and repack/reverify assets after notice changes; see the linked
evidence above.
