# Architecture

Crossroads is a Rust terminal-popup process, not an embedded Herdr extension.
Herdr launches the manifest's platform-specific pane entrypoint with the plugin root
as working directory and supplies runtime environment context. The manifest
declares Linux/macOS/Windows, minimum Herdr 0.9.1, popup placement, and release-binary paths;
it declares a Python binary-installer build hook and the global `setup-keybind`
action. Managed installation invokes the build hook; linking a source checkout
does not. Only explicit invocation of `setup-keybind` may append the agreed host
launch bindings. Both `prefix+f` (Select) and `prefix+a` (Agent) are checked before
any config-content write; a conflict prevents all additions. Correct entries are
retained and only missing entries are appended without rewriting existing bytes.
Opening or searching the popup invokes neither path and does not
install/update binaries or change host configuration. There are no startup or
event hooks. See [installation and setup safeguards](installation.md).

`src/cli.rs` dispatches `--setup-keybind` before runtime initialization, using
`python3` on Unix and `python` on Windows with inherited stdio/environment and
no shell. This keeps one action ID/argv for all platforms despite Herdr's unique-ID
constraint. It requires the published 0.3.1 binary plus manifest/helper;
published 0.3.0 binaries remain immutable and lack this flag. The later Windows
ACL fix in [PR #20](https://github.com/eisenhauer-dev/herdr-crossroads/pull/20)
updates runtime-loaded scripts only: an existing 0.3.1 binary needs no rebuild,
but the pinned `v0.3.1` checkout lacks those updated scripts.

In current source, no argument starts Select; `--agent` initializes the existing
Agent table before the first frame. The runtime applies the launch-pane highlight,
then reconciles it against that table's opening-snapshot membership, falling back
to the first visible agent pane. The manifest adds `crossroads-agent` (Linux/macOS)
and `crossroads-agent-windows` (explicit `.exe`) without changing the Select entries
or 90%×90% popup size. These entries need a rebuilt binary; published 0.3.1 lacks
`--agent`. Combining `--agent` and `--setup-keybind` in either order fails before
setup or runtime initialization. Unknown arguments otherwise remain ignored.

Released 0.2.0 covers Linux x86_64 glibc and macOS 15.0+ ARM64/Intel;
published 0.1.0 stays Linux-only. Published 0.3.1 covers those three targets and
Windows x86_64 GNU. The following Unix distribution evidence does
not cover Windows. Installation selects the running
process architecture, validates ELF/Mach-O and archive/checksum/notice contracts,
and places the payload at the same entrypoint path without Cargo. Native release
helpers bind a fresh final link/map to the exact four-PTY-tested executable;
Darwin ad-hoc signing is link-time only, with no Developer ID or notarization.
CI's real-Herdr local-fixture install gate then tests that installed binary before
artifact upload, without a server/popup. [Current three-platform distribution CI](builds.md#current-020-three-platform-ci-evidence)
passed at that historical pre-release checkpoint. The macOS live
acceptance waiver is not an automated-installation waiver.

### Windows preparation boundary

Native Windows 10/11 x86_64 retains bounded verification limits despite the
published 0.3.1 assets. The manifest separates Unix `crossroads` from Windows
`crossroads-windows` (`./target/release/herdr-crossroads.exe`), with `python` for
Windows installation and `python3` on Unix. The updated `setup-keybind` action
supports both, selecting the appropriate pane launch entry. Its Windows path uses
`scripts/windows-files.py` directory pins and ACL-preserving native replacement
for auto-inherited descriptors, or exclusive-handle in-place append for legacy
descriptors without replacing the file or changing its ACL,
not Unix directory descriptors or mode bits. See the filesystem/race limitations
in [keyboard launch setup](installation.md#keyboard-launch-setup).
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
test-only raw substring assertion on an SGR-styled search label. Run `39614583553`
at `6623013` passed that assertion with the emitted-label matcher, then failed a
stale resize-message expectation despite the correct `80x15` minimum / `60x12`
actual-size notice. Supplied [run 39617733551](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39617733551)
at `b536649` passed all seven native acceptance tests, including resize/close and
theme save/replace/reopen/Auto removal, then failed the distribution suite's
pinned-directory rename assertion. The later
[run 39619790130](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39619790130)
at head `6c464a7` completed successfully: all seven native acceptance checks passed
against both final-linked and installed bytes, and all ten distribution checks
passed, including the corrected pin and rename/swap regressions. Exact artifact
verification also passed; see [build evidence](builds.md#windows-preparation-not-certification).
CI remains fake-host evidence, not actual Herdr integration or Windows certification.
Separately, the user reported successful basic source-build/link/popup use with
Herdr 0.9.5 on a physical Windows 11 PC on 2026-10-28; the exact OS build and
per-function checklist were not recorded. On 2026-11-10 the user also confirmed
successful setup and `prefix+f` launch after the Herdr menu's **Reload config**
action in an updated source/linked checkout (exact Windows build unrecorded).
[PR #20's native GNU CI](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/42831284072)
passed the post-tag ACL append fix, including isolated native integration checks.
These are not all-function, all-version or managed-install live acceptance.
See the [historical MSVC/SDK boundary](builds.md#msvc-and-sdk-attribution-gate)
and current GNU evidence in the build guide.

## Module responsibilities

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Thin executable entry point calling the library |
| `src/lib.rs` | Private module declarations, public `run` facade and unit-test wiring |
| `src/runtime.rs` | Startup and event loop: config, terminal/input probe, subscription-before-snapshot, resize, hints and input deadlines |
| `src/runtime/live_status.rs` | Validated row-status updates, bootstrap draining, bounded batches/redraws, retry eligibility and staged reconnect installation; owns active subscription, retry worker and pending rows |
| `src/config.rs` | Shared JSONC/legacy selection, strict JSON-plus-comments/trailing-commas parsing, duplicate-field locations, grouped diagnostics |
| `src/keymap.rs` | Typed actions and default bindings, schema/replacement validation, shared collision ownership for errors/diagnostics, effective action resolution and labels |
| `src/input.rs` | Classical/Kitty keyboard and bounded SGR mouse byte decoding, key identity/display |
| `src/actions.rs` | Execute a UI key result: focus the emitted pane ID, then redraw only while open |
| `src/focus.rs` | Exact-ID socket focus, bounded reply reading and reply validation |
| `src/topology.rs` | Snapshot validation and pane rows, workspace/tab ordering, own-pane exclusion, sanitized labels, best-effort linked-worktree annotation/grouping |
| `src/transport.rs` | CLI/socket transport, status-subscription ownership and readers, bounded reconnect execution and connection deadlines, reconnect cancellation |
| `src/selection.rs` | Shared UI state, table-view membership, mode/keymap dispatch, pane filtering/navigation and selected pane identity |
| `src/selection/help.rs` | Help entries/state, independent filter and search/navigation input |
| `src/selection/mouse.rs` | Render-recorded hit regions, mouse state/gesture validation, viewport scrolling and existing-action dispatch |
| `src/selection/themes.rs` | Theme-picker input, preview toggle, save/cancel transitions and Auto-transition notice |
| `src/search.rs` | Case-insensitive per-field subsequence matching and greedy source-character match positions |
| `src/theme.rs` | Internal theme facade preserving caller access to palettes, discovery and persistence |
| `src/theme/palette.rs` | Built-in theme names and color roles |
| `src/theme/discovery.rs` | Conservative read-only Herdr Auto theme recognition |
| `src/theme/persistence.rs` | Plugin theme-file validation/diagnostics and explicit save/delete |
| `src/status_colors.rs` | Status-color defaults, file validation/diagnostics, status lookup and explicit overrides of theme roles |
| `src/presentation.rs` | Rendering facade: view dispatch, frame painting, size-only notices, shared text/style helpers and theme notices |
| `src/presentation/table.rs` | Shared search/footer layout, Select table and empty-state messages, dispatch to Agent cards; Select column budgets, styling and viewport |
| `src/presentation/agent.rs` | Agent card content, selected-only metadata, terminal-grapheme clipping, variable-height selection-following and wheel bounds; reused by Agent-origin Themes previews |
| `src/presentation/diagnostics.rs` | Common configuration-error page, source-row cell clipping and diagnostic scrolling |
| `src/presentation/footer.rs` | Effective-binding hint groups, footer row budgets, hint paging and shared search prompts |
| `src/presentation/help.rs` | Help layout, category spacing, selection-following and wheel viewport bounds, wrapped custom bindings and match highlighting; help state/input remains in `selection/help` |
| `src/presentation/themes.rs` | Theme-list picker layout, viewport bounds, visible choices and preview highlight; theme state/persistence remain in `selection`/`theme` |
| `src/terminal.rs` | Terminal sizing, raw-mode, Kitty and mouse ownership/restoration, stdout drawing and cursor placement |
| `src/terminal/windows.rs` | Windows console-mode/VT ownership and restoration, native terminal sizing, no Kitty negotiation |
| `src/terminal/windows_input.rs` | Console input flag policy, shared with portable flag tests and native ConPTY mode tests |
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
`navigation.rs`, `agent.rs`, `help.rs` and `themes.rs`; `tests/unit/selection.rs` wires them
together. `tests/unit/search.rs` covers matching and retained pane/help search,
and `tests/unit/input.rs` covers byte decoding and escape/protocol identities.
`tests/unit/actions.rs` groups cross-context action dispatch and modifier rejection.
Theme cases mirror palette, discovery and persistence ownership under
`tests/unit/theme/`, with the shared layout-fixture check in `tests/unit/theme.rs`.
Validated status-update cases live in `tests/unit/runtime/live_status.rs`, wired
by `tests/unit/runtime.rs`. Controlled coordinator cases in
`tests/unit/runtime/reconnect.rs` cover row installation, selection reconciliation,
opening agent membership and queued status updates; reconnect ordering, staging,
cancellation and focus IPC remain covered by the existing startup process tests. Keymap schema and mixed
configuration/rendering assertions remain in `tests/unit/keymap.rs`. Terminal
lifecycle/output is exercised by startup process and PTY tests; there is no
separate terminal unit-test harness. Process tests and Python PTYs use deterministic
fixtures and fake hosts.

## Data flow

```text
plugin config files ──> config/keymap/theme/status_colors validation ──> Ui
Herdr CLI snapshot + worktree list ──> topology rows ──────────> Ui
stdin bytes ──> Keys decoder ──> context action / search ──────> Ui
rendered layout + mouse event ──> selection/action/viewport ──> Ui
local socket status events ──> validated row status ─────────> Ui
Ui + theme + status_colors + terminal dimensions ──> presentation ──> terminal ──> stdout
Select/Agent confirmation ──> chosen stable pane ID ──> local pane.focus request
Theme confirmation ──> plugin-owned theme file save/delete
```

Rows retain IDs internally; the visible table shows labels rather than IDs.
Rows also retain optional Agent metadata without adding Select display columns. Agent
search includes available repository names, branches and titles; Select search
does not. Titles use only explicit snapshot agent/pane `title` values, never agent
names, pane labels, terminal titles or working directories. Matching agent records
must agree with the pane's workspace, tab and terminal; duplicate associations or
conflicting titles suppress the title. Repository identity/name come only from
workspace `worktree.repo_key`/`repo_name` or a worktree-list `source` paired with a
unique matching `open_workspace_id`; paths are never converted to repository names.
Branches come from those matching worktree entries, not the source workspace;
detached entries supply no branch. Missing/blank values remain absent, conflicting
repository identities or branches are omitted, and duplicate/prunable worktree
associations supply neither repository nor branch. Stored text uses the existing
control-character sanitization. Optional metadata cannot fail popup opening.
It travels with the existing startup/reconnect rows, with no additional CLI calls,
periodic refresh or input-triggered lookup; opening Agent membership is unchanged.
The fake-host card PTY checks these fields from snapshot/worktree responses through
rendering and Agent-only search, including combined status filtering and modal
returns; see [local verification and its platform limits](keymaps.md#local-verification).
The source contract is Herdr v0.9.1's
[`AgentInfo`](https://github.com/herdrdev/herdr/blob/v0.9.1/src/api/schema/agents.rs),
[`PaneInfo`](https://github.com/herdrdev/herdr/blob/v0.9.1/src/api/schema/panes.rs),
[`WorkspaceWorktreeInfo`](https://github.com/herdrdev/herdr/blob/v0.9.1/src/api/schema/workspaces.rs)
and [`WorktreeSourceInfo`/`WorktreeInfo`](https://github.com/herdrdev/herdr/blob/v0.9.1/src/api/schema/worktrees.rs).
Search and navigation operate on visible row indices while remembered selection
is a pane ID. Rendering sanitizes control text. General presentation budgets and
clipping use a conservative estimate: one column per ASCII character and two per
non-ASCII character, not exact Unicode terminal-cell widths. Diagnostic source
paths and Agent card fields instead use grapheme terminal-cell widths when clipping.

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
5. Capture opening agent-pane IDs from detected nonempty row agent names, apply
   the launch-pane highlight, and reconcile Select or the requested Agent view.
   Apply buffered status events and draw. Keymap/status errors
   bypass normal topology startup and show the common error page; independently
   collected theme errors join it. Theme errors alone use Mocha and a notice.

Startup CLI calls are synchronous and are not bounded like reconnect calls.
Do not infer a universal opening-time guarantee from search latency tests.

### Input, filtering, and navigation

An input thread feeds bytes to the main loop. The decoder handles UTF-8,
CSI/SS3 arrows, Kitty input, and a 100-ms raw-Esc ambiguity deadline. Recognized
Ctrl/Alt events cannot invoke actions or become search text; raw HT/CR retain
their unavoidable Tab/Enter aliases.

Select, Agent, Help navigation, and Themes resolve their own effective keymaps to
internal actions and dispatch those actions directly, without synthetic keys.
Existing JSON action names and displayed binding syntax remain unchanged. Pane
Search and Help search instead have fixed text-editing controls. Subsequence
matching is case-insensitive, per field, and preserves the active table/help order.
No CLI call or socket request is caused by a search keystroke. Selection falls
back to the first visible pane when its remembered ID is hidden; empty matches
produce no focus candidate. Navigation is bounded and skips hidden blocks.

The internal `TableView` separates Select (all panes) from Agent (pane IDs with
nonempty detected agent names in the opening rows), independently of the existing
Select/Search/Help/Themes interaction mode. Both table views reuse the same rows,
query and navigation. Select retains topology order. Agent's visible indices put
blocked opening members first using a stable pane-ID queue, without sorting the
shared rows. Initially blocked members enter in topology order; transitions append
newly blocked members, ignore duplicates, remove members leaving blocked, and append
reentries anew. Nonblocked members retain relative topology order. Tracking runs
independently of table view, modal state and search visibility. Row and tab/workspace
block navigation, executable Help actions and Themes previews consume these same
visible indices; block navigation follows display positions, not topology indices.
Status ordering preserves the selected pane ID and never requests host focus.
Changing the table view preserves the selected
pane ID if it is visible, otherwise remembers the first match or no selection for
an empty result; it emits no focus request. Startup defaults to the all-pane view
unless `--agent` is requested.
The `select.open_agent` (`a`) and `agent.open_select` (`s`) actions switch views;
Agent resolves its own keymap with the same navigation actions and defaults.
Search is shared, with printable switch keys remaining text during editing.
Agent-only `filter_all`, `filter_blocked`, `filter_done`, `filter_working`,
`filter_idle` and `filter_unknown` actions default to `a`, `b`, `d`, `w`, `i`, `u`.
The status filter starts at all, intersects the text query, and survives Select,
Help and Themes. Reset-search clears only text; filter-all clears only status.
Live status events reconcile selection for an active Agent status filter to the
retained visible ID, first matching pane, or none. Select's remembered-search
selection behavior is unchanged. Agent search editing also reconciles to visible IDs.
Help lists effective status bindings and dispatches their actions in Agent;
the existing footer does not grow. The Agent badge includes the active status.
The search-row badge and effective footer bindings identify Select versus Agent.
Help carries the calling table's context and switch action; Themes keeps the same
table view for preview and save/cancel return. Select uses the existing table renderer;
Agent uses a focused card body within the same search/footer layout, not a general
mode framework. Switching clears recorded mouse hits, and table-view
and status-filter identity participate in stale-layout validation; pane hits must still be visible.

Default search is lowercase `f` in Select, Agent and Help; custom bindings still replace
it. Any custom assignment to logical Enter is a structured configuration error;
the built-in defaults and fixed Help Enter activation remain available. Help entries
carry their origin action identity. Activation first restores Select, Agent or the actual
Themes list/table state, then dispatches that action, including focus/save/close or
reopening Help. Fixed informational mouse entries carry no action and cannot
dispatch on click/Enter. Ordinary Help closing remains side-effect-free.

Each renderer records mouse hit regions while laying out the actual frame: table
rows reuse the chosen filtered viewport and Ratatui row offsets; Agent cards record
each content row using their actual expanded heights; Help/theme rows
reuse their visible entries; footer positions use rendered terminal-cell widths.
The UI validates the rendered state before using coordinates, and the event loop
rechecks terminal size before mouse dispatch to discard stale resize input.
Mouse defaults on, with a read-only root `mouse_enabled` setting in keymaps.
The wheel changes only the active viewport offset within renderer-supplied bounds,
independent of custom bindings, without wrapping or changing selection, pending
theme choice, search editing state or host focus. It does nothing when all content
fits. Table and Help reuse their viewport offsets; the theme list has a separate
offset so it cannot overwrite the remembered pane viewport. After wheel scrolling,
rendering preserves the offset within the current bounds; keyboard navigation
resumes selection-following. Table layout reconciles the offset to changed viewport
bounds even while following selection, so a taller resize cannot leave an invalid
offset for the next wheel event.
Agent uses card indices for wheel scrolling and sums the actual content heights
plus inter-card gaps when fitting a viewport. Only the selected card adds optional
repository, branch and explicit `Titel:` lines; a title equal to the full location
line is suppressed. Fields clip to one terminal-cell-bounded line, so cards have
at most five content rows. Keyboard navigation and hover keep the selected card
visible; wheel scrolling suspends following. Card selection/expansion invalidates
old mouse regions until redraw. Metadata and label changes also participate in
the layout signature. Repeated motion at the last motion/wheel coordinates is
ignored in card views, preventing stationary-pointer reselection after reflow.

Hover only highlights rows (and previews a pending theme), expanding Agent cards. A single left click
focuses the clicked pane, saves the clicked theme or executes a Help action in
its originating view. Pane hits include theme-table preview and its search.
See [the input and cleanup contract](keymaps.md#mouse-controls), including
theme-preview search return and outstanding native verification.

The presentation facade resets hint paging, then dispatches configuration errors
before the popup-size guard, Help, the theme list, and finally the pane table
(also used for origin-specific Themes table/card preview). Each view uses the existing `Ui` state;
there is no separate renderer state machine. The table chooses compact or roomy
columns, maintains its viewport and underlines matches. Both column layouts use
a full-table-width solid `─` rule below the headers at heights of eight rows or
more; the short ANSI layout omits headers and their rule. Workspace blocks retain
dim dashed `┄` separators, inset before linked worktrees. The same rules apply in
Select Search and Select-origin theme table preview. Agent instead shows agent/status
and `Workspace › Tab › Pane` on every card, without column/group headings.
Shared footer hints stay anchored by each view's
layout. Header labels alone receive explicit italic styling in terminal output;
padding, body and footer do not inherit it. An empty table keeps its frame and
centers the message plus effective reset/switch hints as one block in the body.
Agent distinguishes no surviving opening members from no matching members;
search/preview hints explain how to leave editing/preview before using bindings.
Empty bodies record no pane mouse targets. The main loop checks interactive size
about every 250 ms and pages oversized custom hints every two seconds when needed. Below
80×15 content cells, normal pane actions are blocked; resize can restore the UI.

### Focus and close

Only pane-focus action dispatch with a visible candidate emits a pane ID. `actions`
executes that result through `focus`, which
sends `pane.focus` over `HERDR_SOCKET_PATH` and validates reply ID, result type,
exact pane ID, and `focused: true`. After the request and newline are written,
nonblocking reply reading has a 500-ms overall deadline and a 16-KiB limit.
When no bytes are ready, it sleeps for at most 1 ms before checking the deadline
again; this is not a general socket-connect/write timeout promise.
Stale/error replies fail without choosing another target. Ordinary Help/search
editing, themes, empty results and error-page close do not focus panes; executing
Help's focus action or single-clicking a pane explicitly does. Terminal ownership restores
the saved `stty` state on Unix (native console modes on Windows), shows the cursor,
and pops an owned Unix Kitty mode and disables owned mouse modes on normal
return or error/panic unwinding. Windows enables mouse delivery only together with
VT byte input, not a legacy-only mouse-record reader, and restores the exact saved
console modes. ConPTY's outer tracking sequences are distinct from owned inner modes.

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
Installing reconnect rows reconciles selection against the current table view and
query without recapturing opening agent membership. Disappeared panes therefore
leave Agent results, while newly detected agent assignments enter only on reopen.
The blocked queue retains still-blocked survivors in their previous order, removes
vanished or no-longer-blocked members, and appends newly blocked opening members
in replacement topology order after queued statuses have been applied.
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
