# Architecture

Crossroads is a Rust terminal-popup process, not an embedded Herdr extension.
Herdr launches the manifest's platform-specific pane entrypoint with the plugin root
as working directory and supplies runtime environment context. The manifest
declares Linux/macOS/Windows, minimum Herdr 0.9.1, popup placement, and release-binary paths;
it declares a Python binary-installer build hook and the global `setup-keybind`
action. Managed installation invokes the build hook; linking a source checkout
does not. Only explicit invocation of `setup-keybind` may append the agreed host
launch binding. Opening or searching the popup invokes neither path and does not
install/update binaries or change host configuration. There are no startup or
event hooks. See [installation and setup safeguards](installation.md).

Released 0.2.0 covers Linux x86_64 glibc and macOS 15.0+ ARM64/Intel;
published 0.1.0 stays Linux-only. The following Unix distribution evidence does
not cover the Windows WIP. Installation selects the running
process architecture, validates ELF/Mach-O and archive/checksum/notice contracts,
and places the payload at the same entrypoint path without Cargo. Native release
helpers bind a fresh final link/map to the exact four-PTY-tested executable;
Darwin ad-hoc signing is link-time only, with no Developer ID or notarization.
CI's real-Herdr local-fixture install gate then tests that installed binary before
artifact upload, without a server/popup. [Current three-platform distribution CI](builds.md#current-020-three-platform-ci-evidence)
passed at that historical pre-release checkpoint. The macOS live
acceptance waiver is not an automated-installation waiver.

### Windows preparation boundary

Native Windows 10/11 x86_64 is intended for 0.3.0, **PREPARATION only**, package
still 0.2.0. The manifest separates Unix `crossroads` from Windows
`crossroads-windows` (`./target/release/herdr-crossroads.exe`), with `python` for
Windows installation and `python3` on Unix. `setup-keybind` remains Unix-only.
The Windows installer shares archive/checksum/notice validation but validates
AMD64 PE32+ console headers and uses a native pinned-directory/reparse-rejecting
replacement helper instead of Unix directory descriptors.

`src/terminal/windows.rs` owns native console mode/VT setup and restoration and
terminal sizing; it does not negotiate Kitty. `src/windows_pipe.rs` provides local
byte-mode named-pipe transport for focus and live status, deriving the pipe name
from Herdr's socket marker without opening the marker file. Connect/read/write
paths retain bounded polling and reconnect cancellation. The common UI, byte
decoder, status coordinator and stable pane-ID validation remain shared.

The Windows preparation helper binds a fresh final link/map to the binary tested
with fake CLI/named pipes and ConPTY, then compares installed bytes and repeats
fake-host acceptance. After two native CI job timeouts, the third run verified
bounded cleanup/fail-fast but still failed before terminal entry, with output
bypassing ConPTY. The fourth run (`bcbd93e`) passed the null standard-handle
startup/redirected-parent and exact console-restoration checks, then failed a
test-only raw substring assertion on an SGR-styled search label. The emitted-label
matcher correction awaits native execution; theme-save and installed-binary
acceptance were not reached (see [native test scope](keymaps.md#windows-preparation-verification)).
This is neither actual Herdr integration nor Windows certification. MSVC/SDK
attribution review remains open. A physical Windows PC is available, OS version unknown; final live-popup
acceptance waits **until the pipeline stands**. See [gates and limitations](builds.md#windows-preparation-not-certification).

## Module responsibilities

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Thin executable entry point calling the library |
| `src/lib.rs` | Private module declarations, public `run` facade and unit-test wiring |
| `src/runtime.rs` | Startup and event loop: config, terminal/input probe, subscription-before-snapshot, resize, hints and input deadlines |
| `src/runtime/live_status.rs` | Validated row-status updates, bootstrap draining, bounded batches/redraws, retry eligibility and staged reconnect installation; owns active subscription, retry worker and pending rows |
| `src/config.rs` | Shared JSONC/legacy selection, strict JSON-plus-comments/trailing-commas parsing, duplicate-field locations, grouped diagnostics |
| `src/keymap.rs` | Typed actions and default bindings, schema/replacement validation, shared collision ownership for errors/diagnostics, effective action resolution and labels |
| `src/input.rs` | Classical/Kitty byte decoding and key identity/display only |
| `src/actions.rs` | Execute a UI key result: focus the emitted pane ID, then redraw only while open |
| `src/focus.rs` | Exact-ID socket focus, bounded reply reading and reply validation |
| `src/topology.rs` | Snapshot validation and pane rows, workspace/tab ordering, own-pane exclusion, sanitized labels, best-effort linked-worktree annotation/grouping |
| `src/transport.rs` | CLI/socket transport, status-subscription ownership and readers, bounded reconnect execution and connection deadlines, reconnect cancellation |
| `src/selection.rs` | Shared UI state, mode/keymap dispatch, pane filtering/navigation and selected pane identity |
| `src/selection/help.rs` | Help entries/state, independent filter and search/navigation input |
| `src/selection/themes.rs` | Theme-picker input, preview toggle, save/cancel transitions and Auto-transition notice |
| `src/search.rs` | Case-insensitive per-field subsequence matching and greedy source-character match positions |
| `src/theme.rs` | Internal theme facade preserving caller access to palettes, discovery and persistence |
| `src/theme/palette.rs` | Built-in theme names and color roles |
| `src/theme/discovery.rs` | Conservative read-only Herdr Auto theme recognition |
| `src/theme/persistence.rs` | Plugin theme-file validation/diagnostics and explicit save/delete |
| `src/status_colors.rs` | Status-color defaults, file validation/diagnostics, status lookup and explicit overrides of theme roles |
| `src/presentation.rs` | Rendering facade: view dispatch, frame painting, size-only notices, shared text/style helpers and theme notices |
| `src/presentation/table.rs` | Pane table and theme table preview: column budgets, rows, borders, match/status styling and selected-pane viewport |
| `src/presentation/diagnostics.rs` | Common configuration-error page, source-row cell clipping and diagnostic scrolling |
| `src/presentation/footer.rs` | Effective-binding hint groups, footer row budgets, hint paging and shared search prompts |
| `src/presentation/help.rs` | Help layout, category spacing, selected-entry viewport, wrapped custom bindings and match highlighting; help state/input remains in `selection/help` |
| `src/presentation/themes.rs` | Theme-list picker layout, visible choices and preview highlight; theme state/persistence remain in `selection`/`theme` |
| `src/terminal.rs` | Terminal sizing, raw-mode and Kitty ownership/restoration, stdout drawing and cursor placement |
| `src/terminal/windows.rs` | Windows console-mode/VT ownership and restoration, native terminal sizing, no Kitty negotiation |
| `src/windows_pipe.rs` | Windows byte-mode named-pipe transport for focus/status, bounded polling and cancellation |

Dependencies are the existing pinned JSONC parser, `serde_json`, `ratatui`, and
Unicode general-category tables. There is no plugin SDK, async runtime, database,
or network client. Unit tests live under `tests/unit/` and are included by the
library. Direct status-color theme-override cases live in `tests/unit/status_colors.rs`;
mixed startup and configuration-diagnostic assertions stay in their existing suites.
Rendering cases are grouped under `tests/unit/presentation/` into `table.rs`,
`diagnostics.rs`, `footer.rs`, `help.rs` and `themes.rs`, included as children of
`tests/unit/presentation.rs`. The parent retains shared fixtures/text helpers and
cross-view frame/dispatch checks; the children reuse those fixtures rather than
duplicating a harness. Selection cases live under `tests/unit/selection/` in
`navigation.rs`, `help.rs` and `themes.rs`; `tests/unit/selection.rs` wires them
together. `tests/unit/search.rs` covers matching and retained pane/help search,
and `tests/unit/input.rs` covers byte decoding and escape/protocol identities.
`tests/unit/actions.rs` groups cross-context action dispatch and modifier rejection.
Theme cases mirror palette, discovery and persistence ownership under
`tests/unit/theme/`, with the shared layout-fixture check in `tests/unit/theme.rs`.
Validated status-update cases live in `tests/unit/runtime/live_status.rs`, wired
by `tests/unit/runtime.rs`; reconnect ordering, staging, cancellation and focus
IPC remain covered by the existing startup process tests. Keymap schema and mixed
configuration/rendering assertions remain in `tests/unit/keymap.rs`. Terminal
lifecycle/output is exercised by startup process and PTY tests; there is no
separate terminal unit-test harness. Process tests and Python PTYs use deterministic
fixtures and fake hosts.

## Data flow

```text
plugin config files ──> config/keymap/theme/status_colors validation ──> Ui
Herdr CLI snapshot + worktree list ──> topology rows ──────────> Ui
stdin bytes ──> Keys decoder ──> context action / search ──────> Ui
local socket status events ──> validated row status ─────────> Ui
Ui + theme + status_colors + terminal dimensions ──> presentation ──> terminal ──> stdout
Select confirmation ──> chosen stable pane ID ──> local pane.focus request
Theme confirmation ──> plugin-owned theme file save/delete
```

Rows retain IDs internally; the visible table shows labels rather than IDs.
Search and navigation operate on visible row indices while remembered selection
is a pane ID. Rendering sanitizes control text. General presentation budgets and
clipping use a conservative estimate: one column per ASCII character and two per
non-ASCII character, not exact Unicode terminal-cell widths. Diagnostic source
paths instead use grapheme terminal-cell widths when clipping their source row.

## Core flows

### Opening

1. Load keymaps, status colors, and theme once from `HERDR_PLUGIN_CONFIG_DIR`,
   preferring `.jsonc` and using `.json` only when `.jsonc` is absent.
2. Choose `HERDR_BIN_PATH`, otherwise `herdr`; enter raw mode for terminal stdin.
   Probe Kitty support for up to 100 ms, preserve preceding input, and use
   classical decoding if the response is absent or late.
3. If socket context exists and keymaps/status colors are valid, attempt status
   subscription: CLI `pane list`, then local `events.subscribe` for pane IDs.
4. Obtain CLI `api snapshot`, validate workspace/tab/pane/agent relationships,
   exclude a supplied own-pane ID, order rows, and annotate linked workspaces with
   CLI `worktree list --workspace <id>`. Annotation failures are nonfatal; a
   missing CLI or invalid required snapshot fails opening.
5. Initialize Select, apply buffered status events, and draw. Keymap/status errors
   bypass normal topology startup and show the common error page; independently
   collected theme errors join it. Theme errors alone use Mocha and a notice.

Startup CLI calls are synchronous and are not bounded like reconnect calls.
Do not infer a universal opening-time guarantee from search latency tests.

### Input, filtering, and navigation

An input thread feeds bytes to the main loop. The decoder handles UTF-8,
CSI/SS3 arrows, Kitty input, and a 100-ms raw-Esc ambiguity deadline. Recognized
Ctrl/Alt events cannot invoke actions or become search text; raw HT/CR retain
their unavoidable Tab/Enter aliases.

Select, Help navigation, and Themes resolve their own effective keymaps to
internal actions and dispatch those actions directly, without synthetic keys.
JSON action names and displayed binding syntax remain unchanged. Pane
Search and Help search instead have fixed text-editing controls. Subsequence
matching is case-insensitive, per field, and preserves topology/help order.
No CLI call or socket request is caused by a search keystroke. Selection falls
back to the first visible pane when its remembered ID is hidden; empty matches
produce no focus candidate. Navigation is bounded and skips hidden blocks.

The presentation facade resets hint paging, then dispatches configuration errors
before the popup-size guard, Help, the theme list, and finally the pane table
(also used for theme table preview). Each view uses the existing `Ui` state;
there is no separate renderer state machine. The table chooses compact or roomy
columns, maintains its viewport and underlines matches. Both column layouts use
a full-table-width solid `─` rule below the headers at heights of eight rows or
more; the short ANSI layout omits headers and their rule. Workspace blocks retain
dim dashed `┄` separators, inset before linked worktrees. The same rules apply in
Search and theme table preview. Shared footer hints stay anchored by each view's
layout. The main loop checks interactive size about every 250 ms and pages
oversized custom hints every two seconds when needed. Below
80×15 content cells, normal pane actions are blocked; resize can restore the UI.

### Focus and close

Only confirmation in Select with a visible candidate emits a pane ID. `actions`
executes that result through `focus`, which
sends `pane.focus` over `HERDR_SOCKET_PATH` and validates reply ID, result type,
exact pane ID, and `focused: true`. After the request and newline are written,
nonblocking reply reading has a 500-ms overall deadline and a 16-KiB limit.
When no bytes are ready, it sleeps for at most 1 ms before checking the deadline
again; this is not a general socket-connect/write timeout promise.
Stale/error replies fail without choosing another target. Help, search, themes,
empty results, and error-page close do not focus panes. Terminal ownership restores
the saved `stty` state on Unix (native console modes on Windows), shows the cursor,
and pops an owned Unix Kitty mode on normal
return or error unwinding.

### Live status and reconnection

The local socket reader feeds a bounded 1,024-message channel. Updates must have
the expected event name, known status, and matching pane/workspace IDs on an
agent row; they cannot rename rows or redirect focus. The loop processes bounded
batches and coalesces redraws. Healthy status streaming does not continuously
refresh topology; reopen to refresh it, or a reconnect installs a fresh snapshot.

On disconnect/failure, background retry is eligible after 500 ms. A worker
resubscribes **before** taking a fresh snapshot and worktree annotations, stages
the subscription and rows without constructing another UI, then the main loop
applies queued statuses before installing rows. Active, bootstrap and staged rows
share the same validated status application.
The existing UI remains usable during this work. Reconnect CLI calls have a
two-second deadline per command, cancellation, and capped output; subscription
acknowledgment has a 500-ms deadline. Linux x86_64 also has a bounded socket-connect
retry path; other Unix architectures use ordinary connect. Without a nonempty
socket environment variable there is no subscription or reconnect attempt.
The coordinator owns the row-bearing retry result and drops pending state and
cancels/joins its retry worker before dropping the active subscription; low-level
cancellation and socket shutdown remain owned by `transport`. Transport validates
pane-list IDs locally and does not depend on topology rows.

### Themes and persistence

The theme cursor and pane selection/filter are independent. Space switches the
picker between list and real table preview; cancel discards the preview. Explicit
save writes/syncs a same-directory temporary file and renames it to `theme.jsonc`.
Saving Auto attempts to remove both modern and legacy theme files. A failed
save/delete keeps the picker open and the saved in-memory choice unchanged;
Auto deletion can partially succeed. Keymaps/status colors and Herdr config are
never written. Auto recognizes only a conservative manual built-in Herdr theme,
not live auto-switching, custom palette tokens, or arbitrary TOML.

## Behavior references and evidence

Use [keymaps](keymaps.md), [themes](themes.md), and [status colors](status-colors.md)
for schemas, defaults, error behavior, and terminal limits; use the
[README](../README.md) for setup and measured local search evidence, and
[CONTRIBUTING](../CONTRIBUTING.md) for test layers and complete release gates.
The fake-host tests prove isolated behavior, not live Herdr installation or
cross-platform compatibility. Crossroads performs no network access during open
or search and does not mutate topology, host bindings, or other plugins.
