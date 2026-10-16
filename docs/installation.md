# Binary installation

Crossroads 0.0.2 is the current test-version preparation, with a version-pinned
binary installer and a manifest `[[build]]` hook:
`command = ["python3", "scripts/install-binary.py"]`. It installs the agreed
version into the manifest's fixed `target/release/herdr-crossroads` path. It does
not build, execute the binary, or register/activate a plugin. By default it
downloads the exact archive and checksum from the fixed GitHub Release route
`https://github.com/eisenhauer-dev/herdr-crossroads/releases/download/v<VERSION>/`.
An explicit `CROSSROADS_ASSET_DIR` selects offline local assets instead.
**Version 0.0.2 is not yet published or accepted.** The non-latest
[v0.0.1 prerelease](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.0.1)
is published from reviewed source `1610de9c969a487bb639881cd0c71fe1ee0e3ee2`,
with the exact four reviewed pipeline assets. Its isolated Herdr 0.9.5 public
CLI-install and pinned native single-version tests passed on 2026-10-16, including
opening, search, stable-ID focus, reopening/closing, notices, uninstall and owned
shutdown. All four public downloads matched the approved hashes, and
[main CI run 38871442877](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/38871442877)
passed. The local receipt is `target/public-cycle/0.0.1-verification.json`.
The real 0.0.1 → 0.0.2 → 0.0.1 cycle remains unrun.
There is no short-name registry.

## Requirements and local use

Use a trusted checkout and trusted asset source on Linux x86_64 with glibc and
Python **3.11+**, standard library only. No Rust/Cargo is required to install a
prebuilt binary. Git is required for Herdr's managed source checkout; plugin
runtime still requires `stty` and a running Herdr session (manifest minimum 0.9.1).
Python is an installation/test prerequisite, not a plugin runtime dependency.
No tooling is installed by this helper.

For the current agreed version, the exact files (public or local) are:

```text
herdr-crossroads-0.0.2-x86_64-unknown-linux-gnu.tar.gz
herdr-crossroads-0.0.2-x86_64-unknown-linux-gnu.tar.gz.sha256
```

The [build helper](builds.md) produces these in `target/builds/`. From the trusted
checkout, the following **changes the runtime binary**, but does not link it:

```sh
CROSSROADS_ASSET_DIR=/absolute/path/to/trusted/assets python3 scripts/install-binary.py
```

When set, `CROSSROADS_ASSET_DIR` must be nonempty and absolute. An empty/relative
value or missing local file fails, never falls back to network. Only an absent
variable enables downloads. There is no arbitrary URL override, alternative-version
selection, configurable destination, authentication/token requirement, `gh`, or Cargo
fallback. Do not use `HERDR_PLUGIN_*` for this input: Herdr scrubs those variables
before build hooks, while preserving ordinary custom variables. Close Crossroads
before replacing its binary. Do not run concurrent installers/builds or allow
untrusted processes to modify the checkout or assets during installation.

The existing source-build route remains `cargo build --release --locked`, then
`herdr plugin link "$PWD"`. **Link does not run build hooks**; build or install
the binary first. Linking is user-global registration, not an isolated test.
See [README installation](../README.md#installation) for launch and local-link
cleanup. The isolated [native managed lifecycle test](#native-distribution-acceptance)
passed on Herdr 0.9.3; personal-host setup remains untested. Public CLI-install
evidence is recorded separately above and is not native full-cycle acceptance.

## Validation and failure behavior

The installer reuses `scripts/check-version.py` read-only to require matching
canonical stable versions in Cargo manifest, lock package, and plugin manifest.
The selected archive and its adjacent checksum must exist. Checksum records must
be exactly lowercase SHA256, two spaces, the portable expected filename, and a
newline. Both archive and internal binary hashes are checked.

Public downloads use Python's system TLS trust, a 30-second socket timeout per
request, at most 64 MiB for the archive and 1 KiB for its checksum, and no retries.
HTTPS redirects (including GitHub's signed asset URLs) are permitted; non-HTTPS
or credentialed redirects are rejected before following them. Inherited proxies
are disabled. The expanded gzip payload is limited to 128 MiB in either mode.
Missing release/assets/checksums, HTTP/network/TLS failures and invalid payloads
fail before replacing the previous binary. Network access belongs only to
installation: opening/searching the runtime does not download or update anything.

The gzip tar must contain exactly four regular, non-sparse members named
`herdr-crossroads`, `herdr-crossroads.sha256`, `LICENSE`, and
`THIRD-PARTY-NOTICES.html`. Both notice files must match the trusted checkout's
bytes exactly; missing, tampered or malformed notice payloads fail even if the
outer checksum was recomputed. Older two-member assets are no longer accepted.
Duplicates, unexpected members,
path traversal, absolute names, directories, symlinks, and hardlinks fail. No tar
paths are extracted. The binary must have a little-endian ELF64 x86-64 executable
header; this sanity check is not proof that it runs, is trustworthy, or supports
a particular glibc version.

After validation, all three payload files are staged in exclusive temporary
files in `target/release/`, flushed/synced, then individually atomically replaced:
`LICENSE` and `THIRD-PARTY-NOTICES.html` (mode 0644) first, then
`herdr-crossroads` (mode 0755). Keep both notice files with the installed binary
when copying or redistributing it. Existing symlink/nonregular destinations for
any payload and symlink directory ancestors
are rejected; directory operations are anchored to no-follow open directory
descriptors. Handled failures preserve the previous binary and clean only the
helper's own staging files. A later replacement failure may leave updated notices
beside the previous binary: this is not a three-file transaction. Newly created empty `target/release` directories may
remain after an I/O failure. Forced termination can leave a staging file; this is
not a power-loss durability or rollback guarantee.

Checksums detect corruption, **not publisher authenticity**. Trust both the
checkout (its hook executes code) and asset source. Local artifact evidence is
Linux x86_64 glibc 2.43; no older/public Ubuntu runtime or minimum glibc is proven.
See [build verification limits](builds.md#runtime-and-verification-limits).
The [license inventory](releases.md#license-inventory-and-verification)
covers the active Cargo graph, Rust's upstream standard-library notices, and
exact installed system package copyright/full-license records, plus verified
glibc 2.43-2ubuntu2.4 startup linking exception headers. Final matching binary/link-map
review and repackaged-asset verification remain release gates; those exceptions
do not extend to other sources or versions merely because linkage is dynamic.

## Herdr source resolution

Herdr v0.9.3 and v0.9.5 (also verified in v0.9.1) accept `OWNER/REPO[/SUBDIR]`, generate
`https://github.com/OWNER/REPO.git`, and fetch the supplied `--ref` before
checking out `FETCH_HEAD`. A single slug such as `herdr-crossroads` fails usage;
it does not resolve from a plugin registry.

For the published test prerelease, the public pinned syntax is
`herdr plugin install eisenhauer-dev/herdr-crossroads --ref v0.0.1`
with `CROSSROADS_ASSET_DIR` absent. Set the absolute local override to use trusted
offline assets instead. This selects 0.0.1, not the current 0.0.2 preparation.
Noninteractive callers additionally need `--yes`; review the
source, manifest, and assets before approving hook execution.

Herdr v0.9.3 executes build argv in the manifest root before moving the checkout
and registering with the live `plugin.link` API. In v0.9.5, install moves the
checkout into a managed generation first, builds there, then directly persists
registration in the XDG configuration's `herdr/plugins.json`, even with
a live server. This is the normal v0.9.5 install mechanism, not a connection-error
fallback; installed-plugin lookup still uses `plugin.list`. The registry is a
JSON array of installed-plugin records including enabled state and GitHub source
metadata. A build failure prevents registration and removes the failed managed
checkout; changing the manifest during the build also aborts installation.

Primary source contracts (Herdr **v0.9.3**, checked 2026-10-13):

- [Source parsing, ref checkout, hooks, environment scrubbing and registration](https://github.com/herdrdev/herdr/blob/v0.9.3/src/cli/plugin.rs)
- [Upstream local Git rewrite and failure fixtures](https://github.com/herdrdev/herdr/blob/v0.9.3/tests/cli/plugins.rs)
- [Fake CLI API ping contract](https://github.com/herdrdev/herdr/blob/v0.9.3/tests/cli/harness.rs)
- [Protocol version 22](https://github.com/herdrdev/herdr/blob/v0.9.3/src/protocol/wire.rs)
- [XDG configuration/state roots](https://github.com/herdrdev/herdr/blob/v0.9.3/src/config/io.rs)

Additional primary contracts (Herdr **v0.9.5**, checked 2026-10-15):

- [Install ordering, direct persistence and uninstall](https://github.com/herdrdev/herdr/blob/v0.9.5/src/cli/plugin.rs)
- [Registry JSON array serialization and strict reads](https://github.com/herdrdev/herdr/blob/v0.9.5/src/persist/plugin_registry.rs)
- [Installed-plugin and GitHub source record schema](https://github.com/herdrdev/herdr/blob/v0.9.5/src/api/schema/plugins.rs)
- [Live registry refresh](https://github.com/herdrdev/herdr/blob/v0.9.5/src/app/api/plugins/mod.rs)
- [Upstream managed-installation and shared-registry fixtures](https://github.com/herdrdev/herdr/blob/v0.9.5/tests/cli/plugins.rs)
- [Managed generation leases and reclamation](https://github.com/herdrdev/herdr/blob/v0.9.5/src/plugin_installations.rs)

## Focused verification

From the repository root, `python3 tests/test_install_binary.py` runs isolated
standard-library tests with header-only synthetic ELF bytes. It covers exact
version selection, mocked pinned HTTPS downloads, missing assets/network errors,
download/expansion limits, redirect transport/credential rejection, checksums,
malformed/member attacks, host/ELF rejection,
symlink rejection, and prior-binary preservation on validation/staging/replace
failures. No Herdr or Rust command is invoked and nothing is installed in the
live checkout. Synthetic success is not actual executable verification.
Notice retention, missing/tampered/malformed notice members and notice-destination
symlinks are included. The historical 0.9.3 results below used earlier two-member
assets. On 2026-10-15 both real-Herdr harnesses passed with Herdr 0.9.5 and the
current four-member contract, checking delivered notice bytes and mode 0644.

The separate **main-owned, opt-in** harness requires approval and an explicit real
Herdr **0.9.3 or 0.9.5** executable and actual trusted release assets; other
versions are rejected:

The current default fixtures require matching `0.0.1` assets; historical `0.1.0`
assets are not selected or renamed.

```sh
python3 tests/test_herdr_install.py --herdr-bin /absolute/path/to/herdr --asset-dir /absolute/path/to/trusted/assets
```

Its default mode is not part of ordinary test discovery or CI. It creates only temporary local
Git repos/tags under `/tmp/opencode`, rewrites one fixture GitHub URL to `file://`,
denies every non-file Git protocol, isolates HOME/XDG/config/socket paths, and
inherits no runtime/session/plugin credentials. Its restricted PATH exposes real
Git/Python and denies Cargo/Rust. With 0.9.3, a fake protocol-22 API verifies the
installed binary and notices and echoes source metadata at exactly one
`plugin.link`; no registry file is permitted. With 0.9.5, the API must still
receive `plugin.list` and must not receive `plugin.link`. Instead, the harness
requires exactly one persisted plugin record with the expected ID/version,
minimum Herdr version, enabled state, GitHub owner/repository/ref/resolved commit,
and managed paths within isolated XDG storage, plus exact binary/notice bytes and
modes. It never opens a pane or registers with the current session. Negative asset/checksum/hash/
ELF-target cases must fail before registration, leave no registry file, and leave
no installed runtime. Only session-created fixtures
are cleaned. This tests the real CLI install lifecycle, not a real Herdr server or
plugin runtime, and cannot substitute for live runtime/release gates.

**Execution status (2026-10-13):** main verified all five real-Herdr 0.9.3 CLI
sandbox cases: success, missing asset, missing checksum, bad hash, and wrong ELF
target, using actual locally built release assets. No Rust commands, network,
real registry, or new session topology were used; temporary fixture cleanup
succeeded. The fake API is not a real Herdr server or plugin runtime. Older
v0.9.1 source evidence establishes contracts only, not execution. Isolated
synthetic installer tests also pass. The direct installation, intended managed
install, and host-link examples above remain unexecuted against the personal
environment. This fake-API run alone does not establish native installation;
the later native run below supplies that separately scoped evidence.

**0.9.5 status (2026-10-15):** all five cases passed with the current four-member
assets: success, missing asset, missing checksum, bad hash and wrong ELF target.
The old no-registry assumption was replaced with the upstream version-specific
contract, not with permission to skip registration checks. The real CLI used
only the isolated fake API/local Git fixtures and did not invoke Rust/Cargo or
change the personal installation.

## Public acceptance preparation

The planned test sequence is `0.0.1 → 0.0.2 → 0.0.1`, before the first regular
Linux release `0.1.0`. These need separately built, reviewed and approved public
assets; historical `0.1.0`/trailer-fixture results below do not satisfy that gate.
The fresh [local 0.0.2 build and artifact verification](builds.md) passed;
publication and the actual public update/return cycle remain pending.

Both harnesses also accept an explicit public mode. The pinned 0.0.1 CLI and
native single-version invocations passed. Only the owning main
session may run these after checking isolation and publication approval:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tests/test_herdr_install.py --herdr-bin /absolute/path/to/herdr-0.9.5 --asset-dir /absolute/path/to/reviewed/release-assets --public-ref v0.0.1 --public-commit "$REVIEWED_COMMIT"
PYTHONDONTWRITEBYTECODE=1 python3 tests/test_distribution.py --herdr-bin /absolute/path/to/herdr-0.9.5 --asset-dir /absolute/path/to/reviewed/release-assets --public-ref v0.0.1 --public-commit "$REVIEWED_COMMIT"
```

Set `REVIEWED_COMMIT` to the reviewed full 40-character commit SHA of the approved
tag and replace the absolute path placeholders before use. `--asset-dir` in public mode
holds the independently reviewed reference archive/checksum, **not** an installer
override. The harness removes `CROSSROADS_ASSET_DIR`, removes fixture Git rewrites,
allows only HTTPS Git, and uses the fixed public
`eisenhauer-dev/herdr-crossroads` repository. A canonical `vX.Y.Z` release tag and
full lowercase approved commit SHA are mandatory together. Before installing,
the harness queries the exact `refs/tags/<tag>` and its peeled `^{}` record with
`git ls-remote --exit-code`. Lightweight tags must point directly at the approved
commit; annotated tags must peel to it. Missing, mismatched, duplicate, malformed
or unexpected records and failed lookups abort before any source build hook.
Herdr then installs using **`--ref <approved-full-commit-sha>`**, never the mutable
tag, eliminating a tag-move race between verification and checkout. The persisted
source `requested_ref` must therefore equal the full SHA, and `resolved_commit`
is still checked against that SHA afterward. Native result records distinguish
`verified_release_tag` from the installed `ref` (the SHA).

The reviewed checkout's agreed version still determines the binary asset route:
for version 0.0.1 the production installer fetches the exact archive/checksum
under Release `v0.0.1`, not a SHA-named Release. Binary/notice comparisons and
managed-path checks remain mandatory. No credentials, personal HOME/XDG paths or
inherited Git configuration are used. Herdr must be the explicit real ELF
executable and report **0.9.5**. HOME/XDG isolation is not a filesystem sandbox;
executing the approved commit's hooks still requires trusting its reviewed code.

The CLI harness uses its isolated fake API and checks public install/registration;
the native harness owns a real temporary server/client and checks download-route
installation, source metadata, exact binary/notice bytes/modes, popup navigation,
uninstall, retained state and owned shutdown. Native public mode uses a 200×50 PTY
and the unmodified public popup manifest, not the fixture-only 90% dimensions.
Both keep the restricted no-Cargo/Rust tool PATH and temporary HOME/XDG isolation
under `/tmp/opencode`. Neither creates public tags/releases or mutates the remote.
These single-version invocations test one real version only: they do **not** claim
second-runtime upgrade/return compatibility or run local synthetic negative-asset cases.
The emitted archive hash identifies the reviewed reference archive; binary bytes
are compared with that reference. Local fixture results remain separate evidence.

### Public install/update/return cycle (not executed)

The native harness additionally supports the real `0.0.1 → 0.0.2 → 0.0.1` cycle
on Herdr **0.9.5**. This is separate from both the public single-version pinned
mode above and the offline synthetic `0.1.1` trailer fixture below. **The example
is unexecuted and requires publication, asset review and explicit execution
approval; it is not evidence of public acceptance.** No local version change,
tag, Release, main change or publication is performed by the harness.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tests/test_distribution.py --herdr-bin /absolute/path/to/herdr-0.9.5 --asset-dir /absolute/path/to/reviewed/release-assets --public-ref v0.0.1 --public-commit "$REVIEWED_INITIAL_COMMIT" --public-update-ref v0.0.2 --public-update-commit "$REVIEWED_UPDATE_COMMIT" --public-main-commit "$REVIEWED_INITIAL_MAIN_COMMIT"
```

Replace the placeholders with two separately reviewed full lowercase 40-character
release commits and actual asset paths. `REVIEWED_INITIAL_COMMIT` is the 0.0.1 tag
commit `1610de9c969a487bb639881cd0c71fe1ee0e3ee2`, while
`REVIEWED_INITIAL_MAIN_COMMIT` is the separately reviewed initial main commit
`24e60502de675ca1d6d501ae1cc03421276a03c4`. Recheck that main identity before execution.
`--public-main-commit` is cycle-only and defaults to the initial release SHA when
omitted. The optional `--public-update-main-commit` predeclares the update main SHA
and requires the initial-main flag plus both release pairs. Omit it when the actual
update merge SHA is not yet known; authorize that reviewed SHA at the checkpoint.
All SHAs must be full lowercase 40-character values. The second ref/commit pair is opt-in and requires
the complete first pair. Cycle versions must be exactly `v0.0.1` and `v0.0.2`,
with distinct commits, binary hashes and archive hashes. Both version-specific
archives and their adjacent `.sha256` files must already coexist in `--asset-dir`.
They are immutable reference inputs, not download overrides. No synthetic trailer
is created in either public mode. Before starting the owned server or installing,
both archives/checksums/notices and both public tag-to-reviewed-commit bindings
are checked. Incomplete/floating inputs, missing or unexpected remote records,
and mismatched bindings fail closed.

The owning primary/operator controls the sequence and stdin manually:

1. Obtain approval for execution against the reviewed initial main commit. Hold an
   **exclusive main freeze during the entire initial unpinned operation, including
   checkout and hook execution**. The harness verifies public default `HEAD` is
   `refs/heads/main` at the exact initial reviewed main commit, then runs
   `herdr plugin install eisenhauer-dev/herdr-crossroads --yes` **without `--ref`**.
2. After installation assertions and successful initial native navigation, the
   harness prints a checkpoint naming the required reviewed `0.0.2` commit and
   waits on its original stdin, preserving the same isolated HOME/XDG/session.
   Only after the user separately authorizes that concrete remote `0.0.2` change
   may the primary arrange publication/main transition outside the harness.
   The initial freeze may be released at this checkpoint. Establish an exclusive
   freeze at the reviewed actual update main commit before confirming; it may be
   a merge commit distinct from the approved 0.0.2 release/tag commit.
3. Send exactly `update <REVIEWED_UPDATE_RELEASE_SHA> <REVIEWED_ACTUAL_MAIN_SHA>`
   followed by newline to that stdin. The release SHA must match the supplied
   update release, and any predeclared update main SHA must match the actual main
   SHA. The legacy `update <SHA>` form remains accepted for the predeclared main
   SHA, or the update release SHA when no main SHA was predeclared.
   EOF, other text or an unterminated confirmation aborts without executing
   update. Keep stdin interactive or retain a primary-owned writable stdin pipe;
   do not prefill confirmation, close stdin or use an automatic approval feeder.
   The harness does not poll main or mutate the remote. It verifies the update tag
   against its release SHA and default main against its separate approved main SHA,
   then runs the actual
   `herdr plugin update herdr-crossroads --yes` (no `--ref` exists on update).
4. After update assertions/navigation, the harness returns using the existing
   tag-verified `install --ref <REVIEWED_INITIAL_COMMIT>` path. This return does
   not require moving main back. Uninstall, retained-state and owned shutdown
   checks complete the same session's cycle.

Each unpinned operation has exact main and tag checks before and after execution.
Its `resolved_commit` must equal the approved main SHA; pinned return instead
resolves to the initial release/tag SHA. Tags always bind to release SHAs, not
necessarily to main's merge commits.
**These checks do not eliminate a branch race**: hooks could execute an unreviewed
checkout if main changes between checks, even if it later changes back. Exclusive
main ownership/freeze throughout checkout and hooks is a required external safety
condition, not a guarantee implemented by this harness. Do not run the cycle when
that condition cannot be maintained. Release tags must also remain reviewed and
stable; postchecks detect changes but cannot undo executed hooks.

The source contract was checked against Herdr **v0.9.5**, 2026-10-16:
[`plugin_update` and `git_checkout`](https://github.com/herdrdev/herdr/blob/v0.9.5/src/cli/plugin.rs)
reuse the installed requested ref, fetching `HEAD` when absent;
[`PluginSourceInfo`](https://github.com/herdrdev/herdr/blob/v0.9.5/src/api/schema/plugins.rs)
omits `requested_ref` on serialization when absent. The harness requires that
field to be **absent**, not JSON `null`, after initial install/update, and equal
to the exact initial SHA after return. Resolved commit/version, source identity,
managed path, binary and notice bytes/modes, registry and navigation assertions
remain mandatory at every step, as do retained state, no Cargo/Rust, unchanged
reference assets, uninstall and owned shutdown. Receipts distinguish `install`,
`update`, and `return`, with reviewed release tag and archive/binary hashes;
per-step receipts alone do not prove final cleanup or independent acceptance.

## Local 0.0.1 preflight

On 2026-10-16 the freshly built local `0.0.1` archive passed both isolated Herdr
0.9.5 harnesses: five CLI cases (success, missing asset, missing checksum, bad
hash, wrong target) and the native offline lifecycle with opening, search/focus,
replacement/return, notice retention, uninstall and owned shutdown. HOME/XDG and
the session were temporary under `/tmp/opencode`; personal configuration was not
used. Archive SHA256 is
`93a61fa1972774b7f3c92bc5c1f799752efb3b2a510f3661407971438fabd6c1`, binary SHA256
`4db948b2abaf09c9d9899274fcba8174116130b53245de5e664e6846fd2bd998`.
The replacement still used the explicitly synthetic offline `0.1.1` trailer
fixture, not a real second runtime. Neither public mode nor the actual Herdr
update command was executed. These are local preflight results, not public
acceptance or release approval; the public `0.0.1/0.0.2` gate remains open.

### Pipeline-asset local trial

On 2026-10-16 main repeated both isolated Herdr 0.9.5 harnesses using the exact
downloaded [0.0.1 public pipeline artifact](builds.md#current-001-public-pipeline-evidence):
archive SHA256 `5750e503cfff7447538f92d3b96f4d60f211a1f3788d0420246cd0bc7b76d31e`,
binary SHA256 `145898e7e925df0576af9b124afd8c76c2241f2d5ad64691164ec2eb3a029b41`.
All five real-CLI local cases passed (success, missing asset, missing checksum,
bad hash, wrong target). The native offline cycle
`0.0.1 → synthetic 0.1.1 → 0.0.1` passed opening, search/focus, notice retention,
uninstall and owned shutdown in temporary HOME/XDG/session paths under
`/tmp/opencode`.

This passed the release-procedure **local trial with a public pipeline asset**,
not released-asset installation, public source/download mode, or the real public
`0.0.1 → 0.0.2 → 0.0.1` cycle. The replacement is still the synthetic trailer
fixture; the actual `herdr plugin update` command was not executed. No tag or
GitHub Release existed at the time of this trial. Subsequent 0.0.1 publication
and public CLI-install evidence are recorded above; the full public cycle and
regular release approval remain open. This does not extend the recorded runtime
or license/security scope.

## Dual-profile local acceptance

On 2026-10-16 both opt-in harnesses passed again with Herdr 0.9.5 and the
repackaged dual-builder notice inventory. The CLI harness passed all five local
success/failure cases. The native harness passed installation, popup search and
exact target focus, replacement with the test-only 0.1.1 trailer fixture, return
to 0.1.0, notice retention, uninstall and owned shutdown. Personal configuration
and sessions were untouched; no network or Rust/Cargo ran in the sandbox.

| Recorded asset | SHA256 |
| --- | --- |
| 0.1.0 archive | `753dd33d796804d14474abb50c6cf8745cc82cd51d21f818e9a19662f5fef111` |
| 0.1.0 binary | `146d556e5afee59c61b0664b85457b16b31764d0662062ce553edc4374248c1c` |
| Canonical notices | `08b1ee55eea459afb4e0fe55c68ce91c2f1f815df9d1b9505ffecfdc9226b174` |

This is local distribution evidence, not execution of the public download mode
or a newly released runtime upgrade. At the time, public tag/assets and acceptance remained
pending. Earlier local-only-notice results below are historical evidence.

## Native distribution acceptance

`tests/test_distribution.py` is a separate, opt-in **real Herdr 0.9.3 or 0.9.5 server and
client PTY** test in an isolated named session under `/tmp/opencode`. Only the
owning main session may execute it with explicit approval:

```sh
python3 tests/test_distribution.py --herdr-bin /absolute/path/to/herdr --asset-dir /absolute/path/to/trusted/assets
```

It does not run through ordinary discovery or CI, use the personal session, or
fetch a public repository by default. Explicit absolute Herdr and trusted asset paths are
required. A fixture-only Git URL rewrite maps the selected owner/repository to
local Git data and denies all non-file protocols. Its clean HOME/XDG/environment,
disabled onboarding and host update checks, non-login `/bin/sh`, named sockets and restricted
tool PATH isolate the server and client. The client uses a 160×40 PTY; only the
temporary plugin manifest requests 90% popup dimensions. Cleanup stops only its
owned named session and waits for its owned client/server before fixture removal.

The test verifies exact manifest version, installed binary and notice bytes/modes and source ref/commit,
rendered opening and search, first Enter retaining the filter without host focus,
second Enter focusing the exact stable target pane ID and closing the popup,
replacement and return to the original version, and native uninstall. Close the
popup before replacement or uninstall: uninstall does not stop running plugin
panes. On 0.9.3, managed uninstall must remove its checkout. On 0.9.5, the test
requires an empty persisted registry after uninstall and verifies that the
server-pinned checkout still contains the expected binary and notices. Herdr
0.9.5 retains generation leases until consumers exit; later cleanup can reclaim
unused generations. This harness does not exercise that later reclamation. Both
versions must retain plugin config/state markers; remove only fixture-owned
retained data after shutting down the owned session.

Crossroads supplies no automatic updater. Herdr 0.9.3 has no `plugin update`
command; the opt-in public cycle above exercises the 0.9.5 command. Default offline managed replacement
uses `plugin install OWNER/REPO --ref <reviewed-ref>` again with matching trusted
local assets; returning to a previous ref needs its matching assets too. The
pinned public 0.0.1 CLI install passed separately; the full public cycle remains open.

The current original-version fixture uses the matching local `0.0.1` archive and
its unchanged binary; the historical runs below used `0.1.0`. The temporary
`0.1.1` fixture appends an inert marker to that same runtime
payload to distinguish hashes; it is **test data, not a released 0.1.1 binary or
proof of new-runtime upgrade compatibility**. Results record archive and binary SHA256 values. Version
changes and tags belong only in the temporary fixture repository, never in the
project manifests or project tags. No Rust/Cargo invocation is allowed in the
installation sandbox. Successful per-version results emit ref, resolved commit,
archive/binary SHA256, exact focused pane ID and a test-only version indicator.

Rendering checks read current cursor/erase-updated PTY cells, not historical
transcript substrings, and restrict evidence to the popup query and its first
12 table lines (the fixture allows at most three ordinary root shells). They are
text checks, not screenshots, glyph-fidelity tests or a general terminal emulator.
Unsupported painting controls fail. The known `CSI ? 1016 $ p` pixel-mouse mode
query is non-painting and preserves cells/cursor; an unknown mode-query regression
still fails. Popup opening returns no pane ID: the harness
does not try to read it through ordinary pane APIs. A duplicate open must fail
after the first Enter with structured `ui_busy` and `popup pane is already open`.
After confirmed target focus, a single native `popup.close`
probe must report `popup_not_open`; an Ok response is a failure, not proof of
closure. Reopening an unfiltered popup and closing it with `q` also checks
repeatability before replacement/uninstall.

Risk-free helper validation is `PYTHONDONTWRITEBYTECODE=1 python3
tests/test_distribution_helpers.py` from the repository root. It tests fixture
archive/checksum preparation, fixture-only version changes and the current-cell
reader, without invoking Herdr, Git or Rust. Header-only bytes in these helper
tests do not prove executable behavior. These pure helper tests are a CI
gate; neither real-Herdr harness is included in CI or ordinary discovery.

**0.9.5 execution evidence (2026-10-15):** the real server/client test passed
`0.1.0 → test-only 0.1.1 → 0.1.0` with the current four-member assets. Opening,
search/filter retention, exact target focus, duplicate-open/closed-popup checks,
unfiltered reopening, replacement/return, delivered notice bytes/modes, retained
config/state markers, registration removal and owned-session shutdown all passed.
The server-pinned checkout remained valid after uninstall as required by 0.9.5;
later generation reclamation was not tested. No Rust/Cargo or network ran in the
sandbox, and the personal session was untouched. This local mode was rerun after
preparing public downloads, including all five real-CLI success/failure cases;
six pure helper tests passed before that recorded execution. The hashes below
identify that run, not every later repetition of the generated gzip fixture.
Public-mode execution was still pending at that time. The remaining older
table records historical 0.9.3 evidence.

| Recorded local fixture (2026-10-15) | Binary SHA256 | Archive SHA256 |
| --- | --- | --- |
| 0.1.0 | `146d556e5afee59c61b0664b85457b16b31764d0662062ce553edc4374248c1c` | `64ab7e9ef1f5f0821dda56daf0068566d047e28f0f677125addde1c876275e4c` |
| 0.1.1 (test-only trailer) | `b93fd1d4087f185e8799a79838485cfd2ba103349a5e8e1f9a7b62a5bb99dff6` | `745b4f8650d80ba900ff09522e1d4f1b619b4a3205aad377f81df1592ca6e2d3` |

**Execution evidence (2026-10-14):** the owning main session successfully ran
`PYTHONDONTWRITEBYTECODE=1 python3 tests/test_distribution.py --herdr-bin <absolute-real-herdr> --asset-dir <absolute-trusted-assets>`
from the repository root, after all three helper tests passed. The real Herdr
0.9.3 server and client rendered the popup at each step of
`0.1.0 → test-only 0.1.1 → 0.1.0`. Search showed the sole matching pane row;
first Enter retained the filter and host focus, duplicate opening returned
`ui_busy`, and second Enter focused the exact stable target ID. Native closure,
unfiltered reopening and `q` exit passed. Manifest/ref/local commit, binary mode
0755 and exact bytes were checked at all three steps; returning to 0.1.0 restored
the original hashes/ref/commit.

| Local fixture | Binary SHA256 | Archive SHA256 |
| --- | --- | --- |
| 0.1.0 | `146d556e5afee59c61b0664b85457b16b31764d0662062ce553edc4374248c1c` | `212bf28e4ab0096fee76efbf11ab776433654a72a79ecb99fc740271d8dcb578` |
| 0.1.1 (test-only trailer) | `b93fd1d4087f185e8799a79838485cfd2ba103349a5e8e1f9a7b62a5bb99dff6` | `6e9f5990743d76c7432fb4190354c7d38828d4958baff4d447a42f7df6595fc5` |

Actual CLI uninstall removed registration and the managed checkout while retaining
fixture config/state markers. Source/assets remained unchanged, no Cargo/Rust
commands ran, and owned server/client shutdown left no reachable sockets before
temporary cleanup. No personal session, project version/tag or real network was
used. This proves the defined local managed lifecycle, not a public endpoint,
published tag/asset, personal-host example, newly released runtime upgrade,
older glibc compatibility, independent review or final release acceptance.

The native contracts come from Herdr **v0.9.3**
[named-session paths](https://github.com/herdrdev/herdr/blob/v0.9.3/src/session.rs),
[session snapshots](https://github.com/herdrdev/herdr/blob/v0.9.3/src/app/api/session.rs),
[plugin popup opening](https://github.com/herdrdev/herdr/blob/v0.9.3/src/app/api/plugins/panes.rs),
and [popup lifecycle/tests](https://github.com/herdrdev/herdr/blob/v0.9.3/src/app/popup.rs),
as verified by the owning main session on 2026-10-13. The prior fake-API CLI and
historical navigation checks retain their separately documented scope.
