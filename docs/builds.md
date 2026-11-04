# Build artifacts and verification

The Linux helper retains `x86_64-unknown-linux-gnu`; the macOS helper prepares
`aarch64-apple-darwin` and `x86_64-apple-darwin`. Artifacts are version-named for local release
preparation; building does not publish releases. The build helper makes no tags, publication,
plugin activation, or installation changes. See [release preparation](releases.md)
for the version contract and separate approval/verification boundaries.

**0.2.0 is released** for Linux and macOS. Native Windows 10/11 x86_64 is intended
for **0.3.0**, currently **PREPARATION only**; the checkout's package/manifest
version is **0.3.0**, unpublished on all four targets. Current commands below
prepare new local assets, not public downloads. The historical `0.1.0` Linux
release was published as
[Latest](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.1.0)
on 2026-10-16 with exactly four reviewed clean CI assets, not a prerelease.
Historical local/CI `0.1.0` artifacts below were not reused or renamed.

## Windows preparation (not certification)

The current WIP target is `x86_64-pc-windows-gnu`; native GNU final linking and
PE byte validation succeeded, but the latest native import gate failed before
the bounded preparation exception below was implemented. Native GNU runtime
acceptance, packaging, installation and upload remain **UNRUN**.
The earlier successful receipts below used the retired MSVC target and do not
validate this migration.
The first two native Windows CI attempts
reached the 30-minute job timeout. The third failed at the initial console-handle
check; the fourth passed console restoration but failed a styled-label assertion
in 3m 38s, with clean teardown and no preparation artifact upload.
Those attempts are historical; native Windows CI subsequently passed on
2026-10-21 in [run 39619790130](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39619790130).
The public-only `.github/workflows/windows.yml` selects the standard free public
`windows-2025` runner, PowerShell, Rust/Cargo `1.99.0-x86_64-pc-windows-gnu`,
the existing `C:/mingw64` tools, and Python 3.11+. It has
read-only contents permission, pinned checkout/upload actions, no persisted Git
credentials, and no release publication job. It checks formatting and native
library/binary Clippy before invoking the preparation helper.

On a native Windows x86_64 builder with the exact GNU toolchain described below,
Python 3.11+, Git, installed Rust documentation and cached locked
dependencies, the repository-root command is:

```powershell
python -B scripts/build-windows.py
```

The former MSVC command completed successfully in that run. Earlier attempts are retained
below as failure history, not the current gate status. The first
attempt failed the initial `GetConsoleMode` check before terminal entry, then
hung; the unchanged second attempt also timed out. Supplied evidence from
[run 39603155778](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39603155778)
at `343144f` confirms nine watchdog checks, native Clippy, compilation and selected
Rust suites passed; the console test still failed before terminal entry, but
teardown and fail-fast completed without a hang. Supplied evidence from
[run 39605428457](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39605428457)
at `bcbd93e` confirms the standard-handle correction: redirected-parent and exact
console-restoration checks passed, along with filtered stable-ID focus, live
status reconnect/resnapshot/quit, and noninteractive startup/EOF/missing CLI.
Search then failed because styling split the visible `codex` label in raw bytes.
Supplied evidence from
[run 39614583553](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39614583553)
at `6623013` confirms the SGR-aware matcher passed that search assertion and the
earlier console, focus, reconnect and noninteractive checks. It failed in 2m 56s
at the stale resize expectation `too small`, despite displaying the correct
minimum `80x15` and actual `60x12` content-size notice. The corrected expectation
passed in supplied [run 39617733551](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39617733551)
at `b536649`: all seven native acceptance tests passed, including resize/close and
theme save/replace/reopen/Auto removal. The job failed in 3m 00s during distribution
checks: seven of eight passed, but the pinned-directory rename test did not raise
`OSError`. The installer now requests directory-read as well as attribute access;
that run did not reach installed-binary acceptance. The later successful run
passed the correction and expanded rename/swap regressions, plus both acceptance
passes. See
[the matcher contract and native evidence](keymaps.md#windows-preparation-verification).
The helper installs no tooling. It checks versions and target notices, forces a
fresh final link with a GNU map and double `--trace`, validates PE bytes and
imports against an explicit system DLL allowlist, including the bounded private-contract
preparation exception below, using `objdump -p`, and binds binary/map and captured diagnostic
SHA256 hashes in preparation-only provenance. It runs selected Windows,
input, search and navigation Rust tests, fake-host named-pipe/ConPTY acceptance,
and distribution checks. It packages exactly `herdr-crossroads.exe`,
`herdr-crossroads.exe.sha256`, `LICENSE`, and `THIRD-PARTY-NOTICES.md`, then runs
the Python installer in a temporary checkout, compares installed bytes with the
final-linked executable, and repeats fake-host acceptance on those installed bytes.
It does **not** run an actual Herdr managed install, server, client or live popup.

The workflow checks `tests/test_windows_process.py` and the existing portable
`tests/test_windows_provenance.py` collector regressions with native `python -B`
before native preparation. Collector fixtures do not substitute for native linking.
The shared `scripts/windows-process.py` supervisor logs progress and enforces
command deadlines with bounded process-tree cleanup. Each acceptance method is
isolated in a process with a 30-second deadline, including fixture teardown;
the overall job timeout is unchanged. See [native test scope and timeout limits](keymaps.md#windows-preparation-verification).

Outputs under `target/builds/` currently use
`herdr-crossroads-0.3.0-x86_64-pc-windows-gnu` with `.tar.gz`, `.tar.gz.sha256`,
`.provenance.json`, and `.link.map` suffixes. They derive from the canonical package
version; historical 0.2.0 outputs below remain unchanged. CI uploads exactly
these four files as a
`windows-preparation-` artifact with seven-day retention; these are not approved
release assets. Handled failures after output selection remove the selected
sidecars/archive; stale files never establish a successful invocation.

### Windows GNU toolchain and attribution

The full GNU Rust host is mandatory, including native proc macros, fixture,
unit and ConPTY binaries. Cargo receives an explicit `C:/mingw64/bin/gcc.exe`
linker; the direct fixture `rustc` invocation does too. Both set
`-C link-self-contained=no`: Rust 1.99.0's [codegen option source](https://github.com/rust-lang/rust/blob/1.99.0/src/doc/rustc/src/codegen-options/index.md#link-self-contained)
means the user/linker supplies non-Rust libraries and objects, not Rust's bundled
startup fallback. Rust 1.99.0's [Windows GNU target source](https://github.com/rust-lang/rust/blob/1.99.0/compiler/rustc_target/src/spec/base/windows_gnu.rs)
names `libmsvcrt.a` even with a UCRT-configured driver; this GNU archive is not
the MSVC startup `msvcrt.lib`. Actual PE imports, not its filename, identify DLLs.

Preflight requires GCC **15.2.0**, driver triple `x86_64-w64-mingw32`,
the exact observed approved Binutils build **2.46.0.20260210** for both `ld` and
`objdump`, MinGW-w64 headers **13.0.0**, POSIX threads, SEH and UCRT. Plain
`2.46`, other dated builds and vendor suffixes are rejected, not coerced to a
semantic version or accepted through a `2.46.*` wildcard.
The driver-selected linker must remain in the approved tree and match the hashed
`ld.exe`; GCC, both linker paths and `objdump.exe` are hashed. No package manager,
tool download, setup action or MSVC fallback supplies MinGW. The approved runner
evidence is [runner-images commit a2d4955654f519bd5b559bffaea60d9f90156630](https://github.com/actions/runner-images/blob/a2d4955654f519bd5b559bffaea60d9f90156630/images/windows/scripts/build/Install-Mingw64.ps1),
manifest **20261030.317.1**, checked 2026-11-04. The known upstream
`15.2.0-rt_v13-rev0` release (2025-08-13) has Binutils **2.45**, so it is **not**
an established match for the runner's 2.46 bundle. Provenance therefore leaves
`installed_archive_identity` null rather than inventing an archive hash.

On 2026-11-04, supplied evidence for source `2dd2e329c1fb4d81060c438ed5e12f897ea3037e`,
[run 41905373944, job 126838493831](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/41905373944/job/126838493831),
shows all seven GCC/ld/objdump probes exited zero, but `Verify existing GNU tooling`
rejected the toolchain with the earlier generic error. No build, packaging or
upload was reached. The runner was `windows-2025-vs2026`, image `20261103.298.1`;
its [immutable included-software manifest](https://github.com/actions/runner-images/blob/win25-vs2026/20261103.298/images/windows/Windows2025-VS2026-Readme.md)
reports GCC 15.2 and Binutils 2.46, but the log does not reveal the actual failed
predicate, selected paths or macros. No actual version mismatch is established.
Preflight rejection now adds compact JSON `expected`, `observed` and
`failing_checks`: bounded numeric versions and target grammar, selected-linker
tree membership (no path), thread model, required header values and flag presence.
Unknown values are masked as `absent` or `unsupported`; Binutils diagnostics
retain only the numeric version from the matching tool's first-line `GNU ld` or
`GNU objdump` banner, optionally containing `(GNU Binutils)`. Two to four bounded
numeric components are recognized, including a final eight-digit `YYYYMMDD`
development suffix (bounded month/day fields). Vendor revisions and other unknown
forms remain masked; paths, copyright numbers and arbitrary adjacent text are not
version evidence. The same bounded first-line parser supplies the exact-build
acceptance checks and diagnostics, accepting LF or CRLF. The banner/date layout follows Binutils 2.46's
[ld banner source](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/binutils-2_46/ld/ldver.c),
[shared tool banner source](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/binutils-2_46/binutils/version.c)
and [BFD version generation](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/binutils-2_46/bfd/Makefile.am),
checked 2026-11-05. A linker hash mismatch has a separate
boolean diagnostic. These use existing captures only; apart from the explicitly
approved exact Binutils build binding, acceptance predicates are unchanged.
No raw configuration, unknown paths or full macro dump enters rejection
logs.

On 2026-11-05, supplied evidence for head `1b8ecbae6073d4e2cc834ab6479358796b550cbf`,
[run 41990947095, job 127122779937](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/41990947095/job/127122779937),
shows all GCC version/target, selected-linker tree, thread-model, header, SEH and
UCRT predicates passed. Only `ld_version` and `objdump_version` failed; both probes
exited zero with nonempty captures reported as `unsupported`. Their actual numeric
versions were unknown at that checkpoint: a date suffix was a diagnostic-parser
blind spot, **not** an observed runner version. The diagnostic correction at that
checkpoint still required literal Binutils **2.46** and did not approve dated
versions. Native linking, packaging and upload were not reached. This version
uncertainty is superseded by the later receipt below. The initial checks for this head passed on Linux
(7m 08s), macOS ARM64 (11m 09s), macOS Intel (13m 06s) and PR policy (3s); they
do not establish native GNU Windows acceptance.

Later on 2026-11-05, supplied native evidence for source
`fff5f285c17ae14b40bea0064534d87c9d0a5c6d`,
[run 41997801627, job 127145182556](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/41997801627/job/127145182556),
records both GNU tool banners as exactly **2.46.0.20260210**. All other GCC
15.2.0, target, selected-linker tree, MinGW-w64 13.0.0, POSIX, SEH and UCRT
predicates passed; only the two literal `2.46` checks rejected the builder.
Native GNU linking, testing, packaging and upload were still not reached.
The user then explicitly approved **only this exact observed builder**, applying
the bounded builder-security exception below; fresh native GNU CI and actual
linked-input attribution remain **OPEN**. This supplied receipt was not
independently re-fetched during this correction.

Later on 2026-11-05, supplied evidence for head
`b120e2713adb9f3b433a9710cea7dd05dac7cc13`,
[run 42032581622, job 127262474198](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/42032581622/job/127262474198),
shows the exact approved toolchain preflight, native formatting/Clippy and target
notice check passed. The final `cargo rustc --release --locked` GNU link exited
zero after 38.12s, and PE byte validation was reached without failure. Evidence
collection then rejected `unsupported GNU trace path or member`; the unwrapped
map `LOAD` parser is the identified diagnostic gap. The rejected format is
**unknown**: no relative-path, quoting, library-option or other cause is established.
Native runtime acceptance, packaging, installed-byte acceptance and upload were
not reached; no preparation artifact is claimed. This supplied receipt was not
independently re-fetched during this correction.

Map `LOAD` parse rejection now reports the physical line number and compact JSON
with a fixed reason, structural form and masked shape (at most 120 characters).
Every `LOAD` record contributes to a structural summary, including records after
the rejection; signatures use at most 32 shape characters, with at most 16
signatures and explicit omitted-record counts. The complete JSON is bounded to
2048 characters. Absolute/relative, quote, member, unknown-extension and control
classifications are diagnostic only, not acceptance rules. All names, root paths
and option values are masked, retaining only separators, spaces, quotes,
parentheses and known filename extensions. No raw-file artifact or extra upload is
added.

The next supplied receipt, `d29b935659d80248ce0a10b29fb581819ac66cc8`,
[run 42037286660, job 127278605490](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/42037286660/job/127278605490),
passed preflight, formatting, Clippy, notices, final GNU linking (43.50s, exit zero)
and PE validation. Map line 9709 was rejected as `path_or_member`, with form
`relative|unknown_extension` and shape `? ?`: 104 `LOAD` records comprised 103
absolute object/archive/rlib loads and one two-word value. Its literal value is
**unknown**; this is not confirmation that it was `dll stuff`. Runtime, package,
installation and upload were not reached. This receipt was supplied, not re-fetched.

The collector now supports at most one map-only `LOAD dll stuff` record. Supplied
independent source research pins GNU 2.46 commit
`49d4d3fafa4ec4ff5a3460d91d5b1ed5286487db` (2026-02-08):
[`ld/pe-dll.c`'s `build_filler_bfd`](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/49d4d3fafa4ec4ff5a3460d91d5b1ed5286487db/ld/pe-dll.c#L1096-L1145)
creates the fake `dll stuff` input and an in-memory BFD.
[`ld/ldlang.c`](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/49d4d3fafa4ec4ff5a3460d91d5b1ed5286487db/ld/ldlang.c#L3070-L3180)
distinguishes fake inputs from real file loading/tracing, while its
[map printer](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/49d4d3fafa4ec4ff5a3460d91d5b1ed5286487db/ld/ldlang.c#L4910-L4922)
prints input filenames. This supports the specific upstream format, not identity
with the installed dated builder or confirmation of its actual invocation.

Strict trace validation runs first and is unchanged: even a real file named
`dll stuff` cannot bypass it. All real map inputs must still exactly match the
validated trace inventory. Duplicates, case/whitespace/quote/suffix variants and
unknown relative inputs fail closed with masked diagnostics. The exact synthetic
record is separate evidence, never a filesystem input, archive member or missing
temporary. Raw map/trace hashes, native gates, deadlines and cleanup are unchanged.
The later supplied receipt for `dc4beff67caad0913b28f4f7393bf6bd911d3cf1`,
[run 42044764262, job 127304608117](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/42044764262/job/127304608117)
(2026-11-05), passed preflight, formatting/Clippy/notices and final GNU linking
(29.67s, exit zero). `resolved_link_inputs` completed its map/trace/raw-input
checks, so the exact known synthetic exception matched under the strict inventory
rules. This does not identify the earlier rejected literal retrospectively.
Next, `objdump -p` exited zero but emitted 1,940,764 stdout bytes; the supervisor
retained only the first 1,048,576 and the builder correctly rejected potentially
truncated evidence. Runtime, package, installation and upload were not reached;
no provenance artifact was uploaded or reviewed, so complete attribution is not
claimed. This supplied receipt was not independently re-fetched.

The next supplied native receipt for `ae34ac897bc4b12b135aca0e99a140d6b0bb0d22`,
[run 42054394210, job 127338692055](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/42054394210/job/127338692055)
(2026-11-05), passed preflight, formatting/Clippy/notices, final GNU linking
(39.31s, exit zero) and map/trace/actual-input collection. `objdump -p` exited zero
and its complete capture fit below 4 MiB, then the PE gate rejected an unreviewed
DLL import. At that checkpoint the actual unsupported DLL names were **unknown**;
no runtime, package, installation or upload gate was reached. This supplied receipt was not
independently re-fetched; runtime-DLL hypotheses are not cause evidence.

Only this `objdump -p` call now opts into a 4 MiB per-stream capture, bounded
headroom for the observed output. All other commands, including Cargo JSON, keep
the default 1 MiB per stream. Both stdout and stderr use the selected bound;
required evidence reaching it (even exactly) is rejected, never parsed as partial
output. Disk-backed supervision, bounded memory reads, truncation warnings,
deadlines and cleanup are unchanged. Complete captured PE text still feeds the
same import/delay-import checks and captured/normalized SHA256 fields; that capture
change widened neither the DLL allowlist nor provenance hash semantics. Portable tests cover
real noisy-child capture, tail imports beyond 1 MiB, full-text hashes and
fail-closed boundaries/cleanup. Fresh review, native GNU runtime/package/install/
upload evidence and actual linked-input attribution remain **OPEN**;
`preparation_only: true` and `distribution_approved: false` remain unchanged.

The supplied native receipt at `4575c601fd5d37b0c9b7e552ea9bbdfded124318`,
[run 42092323722, job 127473296934](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/42092323722/job/127473296934)
(2026-11-05), completed GNU linking (20.86s), linked-input collection and full PE
capture, then rejected nine unsupported imports: zero invalid, eight names visible,
one omitted. The visible names were `api-ms-win-crt-environment-l1-1-0.dll`,
`api-ms-win-crt-heap-l1-1-0.dll`, `api-ms-win-crt-locale-l1-1-0.dll`,
`api-ms-win-crt-math-l1-1-0.dll`, `api-ms-win-crt-private-l1-1-0.dll`,
`api-ms-win-crt-runtime-l1-1-0.dll`, `api-ms-win-crt-stdio-l1-1-0.dll` and
`api-ms-win-crt-string-l1-1-0.dll`. The ninth name was **unknown at that checkpoint**. Runtime,
package, installation and upload were not reached. This supplied receipt was not
independently re-fetched; it does not prove private-contract compatibility.

Unsupported PE imports now produce one bounded `diagnostics` JSON error: at most
twelve bare ASCII DLL filenames, each at most 128 characters, plus
`unsupported_records`, `invalid_records` and `omitted_records` counts. Names must
start with an ASCII letter, digit or underscore and otherwise contain only ASCII
letters, digits, underscores, dots or hyphens, ending in `.dll` (case-insensitive).
Paths, option-like strings, spaces, quotes, controls, Unicode and overlong names
are counted as invalid without exposing their contents or path tails. The JSON
payload stays below 2048 bytes; omitted counts cover safe names beyond twelve.
These names are diagnostic evidence only, not additions to the DLL allowlist.
Scanning continues through ordinary imports before rejection, so a later invalid
delay-import directory can take rejection priority; no unsupported import becomes
accepted. Successful provenance schemas, full-capture hashes and cleanup remain
unchanged. The larger diagnostic budget alone does not establish compatibility
or acceptance.

The latest supplied native receipt at `4f6fc2e8de217d9b48ec4cd8569b821bfc24728c`,
[run 42099460689, job 127498111766](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/42099460689/job/127498111766)
(2026-11-05), passed preflight, formatting/Clippy/notices, final GNU linking
(41.03s, exit zero), map/trace/input collection and full PE capture. The import
gate rejected all nine names, with zero invalid or omitted records; the ninth was
`bcryptprimitives.dll`. Runtime/package/install/upload were not reached. This
supplied receipt was not independently re-fetched.

**Approved bounded import exception (2026-11-05):** preparation now accepts only
these nine additional exact names, case-insensitively:

- `bcryptprimitives.dll`
- `api-ms-win-crt-environment-l1-1-0.dll`
- `api-ms-win-crt-heap-l1-1-0.dll`
- `api-ms-win-crt-locale-l1-1-0.dll`
- `api-ms-win-crt-math-l1-1-0.dll`
- `api-ms-win-crt-private-l1-1-0.dll`
- `api-ms-win-crt-runtime-l1-1-0.dll`
- `api-ms-win-crt-stdio-l1-1-0.dll`
- `api-ms-win-crt-string-l1-1-0.dll`

No wildcard, generic API-set prefix, other contract version or additional DLL is
approved. Supplied [Microsoft ProcessPrng documentation](https://learn.microsoft.com/en-us/windows/win32/seccng/processprng)
(2025-07-14, source commit `971ff73ac14a754e46758809d61f39b9610d3030`) names
BCryptPrimitives.dll and Windows 8 desktop as minimum client;
[Rust 1.99.0 source](https://github.com/rust-lang/rust/blob/1.99.0/library/std/src/sys/random/windows.rs)
corroborates ProcessPrng use, not the actual imported function (native symbols
were not dumped). [Microsoft UCRT deployment guidance](https://learn.microsoft.com/en-us/cpp/windows/universal-crt-deployment?view=msvc-170)
describes UCRT as a Windows 10+ OS component; API sets are contracts, not necessarily
physical DLLs. The supplied [MinGW-w64 v13.0.0 private contract definition](https://github.com/mingw-w64/mingw-w64/blob/v13.0.0/mingw-w64-crt/lib-common/api-ms-win-crt-private-l1-1-0.def.in)
supports exact-name existence, **not** a Microsoft public compatibility promise
or the installed builder's source identity. These supplied sources were not
independently re-fetched here. Private-contract Windows 10/11 compatibility
remains **OPEN**, even after a later CI success on one tested host. This is a
user-approved preparation-only exception, not Windows 10 certification,
distribution clearance or advisory-free status. Provenance remains
`preparation_only: true`, `distribution_approved: false`; native GNU runtime,
package/install/upload and actual-input attribution/licensing gates remain open.

The pinned upstream `binutils-2_46` sources define `BFD_VERSION` as `2.46` in
[`bfd/version.m4`](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/binutils-2_46/bfd/version.m4).
[`bfd/version.h`](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/binutils-2_46/bfd/version.h)
describes dates for development versions, excluding release dates;
[`bfd/Makefile.am`](https://github.com/RTEMS/sourceware-mirror-binutils-gdb/blob/binutils-2_46/bfd/Makefile.am)
appends `BFD_VERSION_DATE` to `$(VERSION)` only when `development=true`
(checked 2026-11-05). The observed string alone proves neither source/archive
identity nor patch state: it does not establish a stable release, a confirmed
snapshot or patched advisory status. The notices' `2.46` family label is not
actual source-identity evidence; no notice or exception hash changes follow
from the exact builder approval, and `installed_archive_identity` remains null.

The collector uses GNU ld's [double trace](https://sourceware.org/binutils/docs/ld/Options.html)
(two `--trace` options also print archive members) and cross-checks map `LOAD`
records. Cargo runs with `--message-format=json` and the final binary explicitly
enables `-W linker-messages`. Rust 1.99 captures successful linker stdout and emits
one multiline `linker_messages` warning, not raw trace lines on Cargo stdout
([pinned Rust source](https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/compiler/rustc_codegen_ssa/src/back/link.rs#L1051-L1213)).
The collector selects exactly one `compiler-message` for this checkout's manifest
and binary source/name/kind, then extracts only its warning's `message` field with
the exact `linker stdout: ` prefix and lint code. It does not scrape Cargo stderr,
ANSI-rendered diagnostics, child notes or unrelated build output. The JSON-lines
envelope is documented by the [Cargo revision bundled with that Rust commit](https://github.com/rust-lang/cargo/blob/5f94df4789f005f9a352888e8355ffc645b7ed0e/doc/book/src/reference/external-tools.md#json-messages).
Both input inventory parsing and later readable-input revalidation use the same
extracted UTF-8 bytes. Rust applies `trim_end()` before emitting this diagnostic:
`trace_raw_sha256` hashes the extracted diagnostic payload, **not original linker
stdout bytes**; no trailing whitespace is reconstructed. `trace_capture` records
this limit, while separate diagnostic hashes bind the captured Cargo streams.
Macro preflight and map `OUTPUT` validation accept LF and CRLF line endings
without stripping malformed trailing content. This interpretation does not
rewrite captured text, extracted trace bytes or map bytes: raw hashes retain
their original line endings, and path normalization preserves line endings too.
Trace inputs support only absolute object/archive paths and the two archive-member
forms, including drive letters and spaces. The map-only exception above does not
expand that trace grammar. Unknown trace syntax,
missing maps/traces, missing/duplicate matching diagnostics, malformed JSON,
either diagnostic stream reaching the 1 MiB capture limit, missing external startup/runtime
inputs or mismatched inventories fail closed. Required `crt2.o`, `crtbegin.o`,
`crtend.o`, `libmingw32.a`, `libmingwex.a`, `libmsvcrt.a`, `libgcc.a`,
`libgcc_eh.a` and `libpthread.a` must be readable under the approved MinGW tree.
Other unavailable inputs are accepted only as recognizable rustc temporary
objects/import archives and marked `missing_temporary_after_link`, without hashes.
Opened files and reported archive members are separate; neither is a guessed
complete symbol-contribution inventory. Symbol completeness remains unknown.

Schema 2 adds `resolved_link_inputs.synthetic_inputs`: an empty list or one record
with `name: "dll stuff"`, `kind: "gnu_pe_dll_filler"` and `map_line` (the physical
map line, for LF or CRLF). It has no filesystem path, availability or hash. The
unchanged raw-map hash binds the original bytes including this record; no synthetic
file hash or source identity is invented.
Schema 2 binds the exact PE, raw/normalized map, trace, captured diagnostics,
tool hashes, source commit/status and PE import diagnostics. Personal repository,
home, Cargo and Rustup roots are normalized; trace rejection diagnostics disclose only
the physical trace line number. Captured diagnostic hashes cover retained bytes
only and stream completeness remains unknown. `objdump -p` accepts only the
explicit system DLL allowlist with the bounded private-contract preparation
exception above and zero delay-import directory: unexpected
MinGW runtime DLLs, `VCRUNTIME`/`MSVCP` and foreign DLLs fail. OS `msvcrt.dll` is
allowed, distinct from MSVC startup code. Four archive members remain mandatory;
no compiler or runtime DLL is distributed. Byte/hash checks reject binary, map,
provenance, tool or readable input changes during testing; installed bytes are
checked again after acceptance. Failures remove selected preparation outputs.

Notices now derive the **82-package** Linux/GNU Windows union, including
`winapi-x86_64-pc-windows-gnu 0.4.0`. That published crate omitted license files;
the generator checks exact upstream workspace MIT/Apache license hashes using
cached `winapi 0.3.9` texts, not its unrelated binary `libwinapi_oemlicense.a`.
The generator reproduces the full MinGW-w64 v13 runtime omnibus and winpthreads
notices, preserving their exact source bytes. GCC 15.2.0's
[runtime source](https://github.com/gcc-mirror/gcc/blob/releases/gcc-15.2.0/libgcc/libgcc2.c)
and [Exception 3.1](https://github.com/gcc-mirror/gcc/blob/releases/gcc-15.2.0/COPYING.RUNTIME)
permit eligible target code with independently licensed modules under their own
terms; GCC itself is not MIT. GPLv3 and Exception 3.1 are included in the existing
system notices. The MinGW omnibus itself retains unresolved component notes;
native input/member attribution and applicable licenses still need review.
Upstream notice inclusion is not proof of the runner bundle or actual consumption.

**Approved bounded builder-security exception (2026-11-04; exact builder approved 2026-11-05):**
GCC 15.2.0 and the exact observed Binutils **2.46.0.20260210** builder are not
claimed advisory-free. The owning session's Binutils 2.46-family/GCC exact-CPE NVD
checks found GCC [CVE-2023-4039](https://nvd.nist.gov/vuln/detail/CVE-2023-4039)
(medium, AArch64 rather than this target), Binutils
[CVE-2025-69649](https://nvd.nist.gov/vuln/detail/CVE-2025-69649) and
[CVE-2025-69650](https://nvd.nist.gov/vuln/detail/CVE-2025-69650)
(readelf 7.5; the latter disputed/pre-release), medium CVE-2025-69651/69652,
and [CVE-2026-6846](https://nvd.nist.gov/vuln/detail/CVE-2026-6846)
(crafted XCOFF linker input, 7.8). The approved scope is project-only PE inputs,
no foreign binaries, no `readelf`, and no toolchain redistribution. These are
builder risks, not demonstrated plugin runtime CVEs. Approval of this exact build
is not blanket security or release clearance and does not establish advisory
applicability or patch state. MinGW-w64 13.0.0's exact-CPE
query returned zero records, a limited query and not a safety guarantee.

Native GNU execution cannot be performed on this Linux builder: no local MinGW
tools or GNU Rust target are installed. Portable tests do not waive fresh native
GNU CI, independent review or release approval. Rollback means restoring a
separately reviewed MSVC source/toolchain configuration, never renaming historical
artifacts as GNU; the historical MSVC licensing question would remain open.

### Successful Windows preparation evidence

On 2026-10-21, run `39619790130`, job `119289869684`, completed successfully in
3m 14s for head `6c464a799e826d246e83357c01ab4298f406c79d`. The actual PR checkout
was synthetic merge `5d5b8c4f9d98e81c0d29f8862a1508e64a2b6826`, with head and base
parents, not a claim that CI directly built the head SHA. All seven native
acceptance checks passed twice: on the exact final-linked binary, then the
byte-identical installed binary. All ten distribution checks passed, including
pinned-parent rename, junction swap and rejection of an existing DELETE handle.
The watchdog suite's one POSIX-only skip was not a bypassed acceptance test.
The runner reported Windows Server 2025 build `10.0.26100`, not Windows 10/11.

Artifact `12353319857`,
`windows-preparation-herdr-crossroads-0.2.0-x86_64-pc-windows-msvc`, contained the
four expected outer assets. Read-only in-memory verification matched the ZIP
digest, archive/checksum, binary/map bindings, exact four inner archive members
and canonical notice bytes. The 890,237-byte ZIP had SHA256
`b33d82b2bf188ac18b1cfcc1cb355c6da030dcccefde248eb7764d812da7df95`.

| Windows preparation evidence | SHA256 |
| --- | --- |
| Binary | `60c87612c6bf7717abff11b702b3e0c34b25be6b057958b0581cd8b9d0e104b9` |
| Normalized link map | `6ac6c964d314a9c7b1709b803571a5a92df548f093f7dfc5b8523a5499c66089` |

The artifact's seven-day retention expires at `2026-10-28T14:49:38Z`; this is a
historical receipt, not a persistent download route. Its provenance remains
`preparation_only: true`, `distribution_approved: false`, with the original
MSVC/SDK, Windows 10 and real-Herdr open gates. Later manual evidence does not
rewrite that historical artifact.

On 2026-10-28, the user separately reported successful basic real-Herdr 0.9.5
source-build/link/live-popup use on a physical Windows 11 PC at the same local
`develop` head. The OS was described as the latest Windows 11, without an exact
build or per-function checklist. This is user-reported basic manual acceptance,
not automated hardware testing, full-feature certification, Windows 10 coverage
or Herdr managed-install acceptance. See [launch details](installation.md#windows-preparation-limitations).

On 2026-11-04 the user accepted the existing Windows tests as sufficient and waived
additional Windows 10/11 managed-install/live-popup testing. This is a testing
waiver, not a new run, Windows 10 coverage or certification. The
[latest candidate receipts](releases.md#latest-candidate-evidence-and-testing-acceptance)
retain exact source/run scope, the completed ARM64 retry and final-tree CI requirement.

**Open gates:** native GNU CI and GNU linked-input attribution/licensing review;
independent combined review, primary acceptance and separate
release approval. No approved Windows release or
managed-install route exists. Neither the runner label nor synthetic PE tests
establish Windows 10/11 support.
See [installer and launch limitations](installation.md#windows-preparation-limitations)
and [native test scope](keymaps.md#windows-preparation-verification).

### MSVC and SDK attribution gate

This section preserves the **retired MSVC collector and historical receipts**.
It is not the current GNU collector contract or evidence for GNU binaries.

Research checked on 2026-10-28 did **not** establish license clearance. The historical
0.2.0 artifact's link diagnostics identify Visual Studio 18 Enterprise / MSVC
`14.51.36231`, but do not establish the exact Windows SDK version, resolved
library paths or applicable license provenance. That `/VERBOSE:LIB` capture is
not a complete linked-input attribution inventory and is not retroactively improved.

The retired MSVC collector additionally requested
[`/LINKREPROFULLPATHRSP`](https://learn.microsoft.com/en-us/cpp/build/reference/link-repro-full-path-rsp?view=msvc-180),
documented by Microsoft as available since Visual Studio 2022 17.11 (source updated
2025-03-31, checked 2026-11-02). It embeds the normalized, linker-reported absolute
inputs and response text in `resolved_link_inputs`, with raw/normalized response
hashes. It records post-link file hashes when readable (`read_after_link`) and
marks removed temporary inputs `missing_after_link`. Exactly one MSVC toolset
version and one Windows SDK version must be identifiable from the reported path
components; missing/ambiguous versions, missing responses and unrecognized response
lines fail rather than guessing from installed directories or searched candidates.
Besides plain quoted absolute paths, the parser accepts only the native-observed
whole-quoted `"/DEFAULTLIB:<absolute-path>"` and `"/NATVIS:<absolute-path>"` forms,
with ASCII case-insensitive option names. Relative values (including
`/DEFAULTLIB:user32.lib`), unknown switches and malformed quoting still fail;
there is no library-directory or environment search fallback.

`resolved_link_inputs.files` retains plain paths and explicit absolute DEFAULTLIB
library paths. Each record now adds a canonical `source` (`quoted-path` or
`/DEFAULTLIB`) and 1-based `response_line`. The separate `debugger_metadata`
array records `/NATVIS` paths with the same source/line, normalization, hash and
availability fields; it is empty when no NATVIS arguments are reported. Only
`files`, never debugger metadata, supplies SDK/toolset version candidates. These
are additive fields within the preparation provenance's existing schema version 1;
the complete normalized response, its hashes and binary/map bindings remain.

Microsoft documents [DEFAULTLIB](https://learn.microsoft.com/en-us/cpp/build/reference/defaultlib-specify-default-library?view=msvc-180)
as a library to search for external references. A reported absolute DEFAULTLIB
path is retained as linker-reported library evidence, not proof that any symbols
were incorporated. [NATVIS](https://learn.microsoft.com/en-us/cpp/build/reference/natvis-add-natvis-to-pdb?view=msvc-180)
requests debugger visualizations in a PDB and is ignored when no PDB is created;
the metadata records establish neither code incorporation nor PDB embedding.
Both references and Microsoft's case-insensitive option-name rule in the
[linker syntax reference](https://learn.microsoft.com/en-us/cpp/build/reference/linking?view=msvc-180#command-line)
were checked for `msvc-180` on 2026-11-04. Missing reported option files retain
`missing_after_link` without invented hashes, just as other reported inputs do.
The temporary `.link.rsp` is removed after embedding, not uploaded as a fifth asset.
Rejected lines now report their 1-based response line number and distinguish an
unexpected format from a quoted path that is not absolute. The JSON-escaped
`shape` preview preserves quotes, separators and normalized `<repo>`, `<home>`,
`<cargo>` and `<rustup>` roots, but masks remaining text, including unknown path
components and option names/values. Previews longer than 120 characters retain
only the first 80 and last 40, separated by `...`, before escaping controls.
This is structural context, not the original line or a format acceptance change.
On rejection, a separate JSON `summary` scans the whole response and reports its
line count and up to 16 distinct signatures in first-seen order, with each
signature's first line and total occurrence count. Signatures distinguish blank
lines, quoted/unquoted absolute paths, quoted nonabsolute paths, other text and
syntactic options. Only a leading `/` or `-` option identifier is exposed, after
removing a complete outer quote pair: an ASCII letter followed by at most 31
ASCII letters, digits, `_` or `-`, delimited by `:` or the end of the line.
Identifiers are uppercased; values are never included. Drive-letter file paths
are not option identifiers. This is syntax recognition, not an option allowlist
or evidence that any option denotes a consumed file. The summary's serialized
JSON is capped at 2,048 characters; exceeding either limit sets `truncated: true`
and omits later signatures. Counts for retained signatures still cover the whole
response. The existing escaped `shape` continues to mask option names and values.
Failure cleanup still removes the raw response and selected outputs; no raw file
is persisted or additionally uploaded.

These are native-reported linker **inputs**, not an inventory of incorporated
symbols; symbol-contribution completeness is unknown. `linker_executable_version`
remains null: a toolset path is not the exact linker executable version. The
normalized stdout/stderr capture and its raw/normalized hashes are bound alongside
the binary/map hashes, but capture is limited to supervisor-retained prefixes;
stream completeness is unknown and raw diagnostic bytes are not persisted. Portable
fixtures cover collection and fail-closed cases, not observed 0.3.0 inputs. In
[native run 41798627194, job 126468652020](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/41798627194/job/126468652020),
the final link exited successfully but collection rejected a response input with
the earlier generic error. The later
[run 41801924450, job 126479512155](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/41801924450/job/126479512155)
reported line 1 as nonabsolute, with a quoted option-and-path shape. Its exact
option name was masked; that evidence did not establish `/LIBPATH`. The later
[run 41821512175, job 126545532475](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/41821512175/job/126545532475)
reported 88 response lines: one whole-quoted `/DEFAULTLIB` argument at line 1,
four whole-quoted `/NATVIS` arguments starting at line 2, and 83 quoted absolute
paths starting at line 6, with no summary truncation. Its final link succeeded
but the earlier parser rejected line 1. The corrected parser subsequently passed
[native run 41830572270](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/41830572270)
on 2026-11-04, including final-linked and installed-binary acceptance and upload.
The inspected 0.3.0 provenance records MSVC toolset `14.51.36231` and Windows SDK
`10.0.26100.0`, 84 reported library/object inputs (81 readable after linking and
three removed temporary inputs), and four separately hashed NATVIS metadata files.
These versions come from input paths, not an exact linker executable FileVersion.
The clean source is PR merge `1449de3d26a09a245e717717bb09cf438535b946`, whose
parents are `main@46f2e136` and `develop@676c087`, not a direct build of the head SHA.
The archive SHA256 is `99d7fbbc95127c4a6242c0c32161662bd7b3a084cf42ead733a1f8d24132b138`;
the binary SHA256 is `d0897f407f02d120cf7bc03017cb28a9695c5231cf8f400363d40ea42d07a7d8`.
Archive/checksum/PE and binary/map provenance checks passed historically. The
Windows archive's `LICENSE` uses CRLF, not the LF bytes now required by
`.gitattributes` (`LICENSE text eol=lf`); the byte-exact installer rejects that
archive against the canonical checkout. `THIRD-PARTY-NOTICES.md` remains
byte-identical under its deliberate `-text` policy. Preserve this archive and its
receipt as historical evidence, not a final-policy candidate; fresh final-tree
assets and native package/install checks are required. The portable real-Git
regression covers `core.autocrlf=true`/`false` producer and consumer checkouts in
both directions and still rejects changed license content; running it on Linux
is not native Windows installation evidence. Preparation-only flags remain
unchanged; successful collection does not close licensing or establish Windows
10/11 certification. The later testing waiver does not rewrite this provenance.

Microsoft's full-path response example above contains quoted file paths only.
Its [general linker command-file syntax](https://learn.microsoft.com/en-us/cpp/build/reference/linking?view=msvc-180#linker-command-files)
allows options and filenames, but does not establish the generated response's
complete grammar or the native option set. Both references were checked for
`msvc-180` on 2026-11-04. Native evidence and the option-specific references above
support the two narrowly recognized forms, not a general command-line parser.
Any further unknown form still fails with bounded diagnostics. Portable fixtures
use illustrative paths/versions, not recovered native inputs or license evidence.

Verified research supplied by the owning session on 2026-11-04 found CRT
startup/support from `msvcrt.lib` incorporated in the actual map/PE, including
`mainCRTStartup` and `__scrt_common_main_seh`. `VCRUNTIME140.dll`, `api-ms-win-crt-*`
and OS DLLs are imported, not bundled: this is neither a fully static CRT nor an
import-only input inventory. Microsoft's [CRT library documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/crt-library-features?view=msvc-180)
describes `msvcrt.lib` native startup for dynamic UCRT/vcruntime.

The operative [Visual Studio 2026 Enterprise/Professional EULA](https://visualstudio.microsoft.com/wp-content/uploads/2025/10/Visual_Studio_2026-License-Enterprise_Professional_ENU.docx)
(2025-10-01) grants redistribution for Distributable List code, with conditions
including significant functionality, downstream protection terms and indemnity.
The [2026 redistribution list](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution)
does not expressly resolve startup objects in `VC/Tools/.../lib/msvcrt.lib` versus
listed `VC/redist` runtime files. Applicability remains unresolved; do not impose
speculative Microsoft end-user conditions or infer redistribution clearance.
The [Windows SDK license](https://learn.microsoft.com/en-us/legal/windows-sdk/license)
and [SDK redistribution list](https://learn.microsoft.com/en-us/legal/windows-sdk/redist)
also require applicable component review. These supplied findings were not
independently re-researched during this documentation update.

Public sources do not establish that a user must independently buy an Enterprise
subscription for the GitHub hosted runner. The provider-internal contract is
unverified, a limitation rather than a demonstrated user license defect. Exact
linker FileVersion and a full-symbol inventory have not been identified as legal
prerequisites. The attribution/licensing gate remains open, not a finding that
MSVC linking is prohibited. No `LICENSE`, notice or preparation flag is changed;
further executable evidence or CI requires separate scope/approval.

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
diagnostics retain captured output. The helper flushes each gate's start marker,
captured output on success, and passed/failed elapsed time (monotonic seconds)
before proceeding. Output within a running gate remains captured until it exits;
these measurements attribute native gate cost, not a CI performance improvement.
The subsequent [keymap post-exit drain optimization](keymaps.md#local-verification)
removed 80 redundant 200-ms quiet waits per complete keymap run. On local Linux
on 2026-10-28, two unprofiled runs took 51.144/51.290 seconds before and
34.975/34.924 seconds after, against the unchanged release executable (SHA256
`0c93bab560a765502000f351e8ae52ee6d9532b289efb075f6efffa6ecedcc2e`).
Separate bounded operation counts measured 16.032 seconds in post-exit drains
before and 0.002 seconds after, with all 80 popups and four interactive drains
retained. These are local harness measurements, not native macOS timings or a
claim that CI is under ten minutes. Native final-linked and byte-identical
installed-binary verification remains pending for this change.
CI relies on these final-binary gates instead
of a duplicate ordinary release build/PTY pass, then retains the separate
installed-package real-Herdr acceptance before upload. The job limit remains
30 minutes. Link-time ad-hoc signing occurs **before** testing;
there is no post-test signing, stripping or other binary mutation. These are
**not Developer ID signed or notarized** assets; ad-hoc signing and SHA256 do not
authenticate a publisher or establish Gatekeeper acceptance.

For each target, current local preparation output under `target/builds/` uses
`herdr-crossroads-0.3.0-<target>` with `.tar.gz`, `.tar.gz.sha256`,
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
target/builds/herdr-crossroads-0.3.0-x86_64-unknown-linux-gnu.tar.gz
target/builds/herdr-crossroads-0.3.0-x86_64-unknown-linux-gnu.tar.gz.sha256
target/builds/herdr-crossroads-0.3.0-x86_64-unknown-linux-gnu.provenance.json
target/builds/herdr-crossroads-0.3.0-x86_64-unknown-linux-gnu.link.map
```

These names use the checkout version `0.3.0`, not a fixed literal in the
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
new 0.3.0 outputs are separate artifacts from both published 0.1.0 and 0.2.0.
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
(cd target/builds && sha256sum -c herdr-crossroads-0.3.0-x86_64-unknown-linux-gnu.tar.gz.sha256)
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

The public-only **Linux · Build & Tests (x86_64)** CI job builds the same target on
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
