# Binary installation

Crossroads 0.1.0 has a version-pinned binary installer and a manifest `[[build]]` hook:
`command = ["python3", "scripts/install-binary.py"]`. It installs the agreed
version into the manifest's fixed `target/release/herdr-crossroads` path. It does
not build, execute the binary, or register/activate a plugin. By default it
downloads the exact archive and checksum from the fixed GitHub Release route
`https://github.com/eisenhauer-dev/herdr-crossroads/releases/download/v<VERSION>/`.
An explicit `CROSSROADS_ASSET_DIR` selects offline local assets instead.
**Prepared, not publicly accepted:** as of 2026-10-15 no release tag or Release
exists. The public route cannot succeed until the matching reviewed checkout,
tag and assets are published; publication and public acceptance remain pending.
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
herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz
herdr-crossroads-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
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
passed on Herdr 0.9.3; personal-host setup and public distribution remain untested.

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

For the selected source, the prepared public pinned syntax is
`herdr plugin install eisenhauer-dev/herdr-crossroads --ref v0.1.0`
with `CROSSROADS_ASSET_DIR` absent. Set the absolute local override to use trusted
offline assets instead. **This is pending syntax, not a currently runnable public
installation instruction or an existing public release.** Noninteractive callers additionally need `--yes`; review the
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

Both harnesses also accept an explicit public mode. **Not executed; blocked on
an approved published tag/Release and its reviewed commit.** Only the owning main
session may run these after checking isolation and publication approval:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tests/test_herdr_install.py --herdr-bin /absolute/path/to/herdr-0.9.5 --asset-dir /absolute/path/to/reviewed/release-assets --public-ref v0.1.0 --public-commit "$REVIEWED_COMMIT"
PYTHONDONTWRITEBYTECODE=1 python3 tests/test_distribution.py --herdr-bin /absolute/path/to/herdr-0.9.5 --asset-dir /absolute/path/to/reviewed/release-assets --public-ref v0.1.0 --public-commit "$REVIEWED_COMMIT"
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
for version 0.1.0 the production installer fetches the exact archive/checksum
under Release `v0.1.0`, not a SHA-named Release. Binary/notice comparisons and
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
Public mode tests one real version only: it does **not** claim second-runtime
upgrade/return compatibility or run the local synthetic negative-asset cases.
The emitted archive hash identifies the reviewed reference archive; binary bytes
are compared with that reference. Local fixture results remain separate evidence.

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
command; 0.9.5 has one, but it is not exercised by this harness. Managed replacement
uses `plugin install OWNER/REPO --ref <reviewed-ref>` again with matching trusted
local assets; returning to a previous ref needs its matching assets too. The
pinned public syntax above remains pending publication and acceptance.

The original 0.1.0 fixture uses the validated local archive and its unchanged
binary. The temporary 0.1.1 fixture appends an inert marker to that same runtime
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
Public-mode execution is still pending. The remaining older
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
