# Architecture

Crossroads is a Rust terminal-popup process, not an embedded Herdr extension.
Herdr launches the manifest's `crossroads` pane entrypoint with the plugin root
as working directory and supplies runtime environment context. The manifest
declares Linux, minimum Herdr 0.9.1, popup placement, and a release-binary path;
it has no build hooks, actions, startup hooks, or event hooks.

## Module responsibilities

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Thin executable entry point calling the library |
| `src/lib.rs` | Startup orchestration and event loop: config, terminal/input probe, snapshot, status subscription, resize, redraw, reconnect lifecycle |
| `src/config.rs` | Shared JSONC/legacy selection, strict JSON-plus-comments/trailing-commas parsing, duplicate-field locations, grouped diagnostics |
| `src/keymap.rs` | Default bindings, schema/replacement validation, per-context collision checks, effective action resolution and labels |
| `src/input.rs` | Classical/Kitty byte decoding, key identity/display, key dispatch/redraw, exact-ID socket focus and reply validation |
| `src/topology.rs` | Snapshot validation and pane rows, workspace/tab ordering, own-pane exclusion, sanitized labels, best-effort linked-worktree annotation/grouping |
| `src/transport.rs` | CLI/socket transport, status-subscription ownership and readers, bounded reconnect execution and connection deadlines, reconnect cancellation |
| `src/selection.rs` | UI state machine, independent pane/help filters, subsequence matching, visible navigation, selected pane identity, theme preview/save, validated status updates |
| `src/theme.rs` | Built-in palettes, conservative Auto recognition, theme-file validation and explicit persistence |
| `src/status_colors.rs` | Status-color defaults, file validation/diagnostics, status lookup and explicit overrides of theme roles |
| `src/presentation.rs` | Rendering facade: view dispatch, frame painting, size-only notices, shared text/style helpers and theme notices |
| `src/presentation/table.rs` | Pane table and theme table preview: column budgets, rows, borders, match/status styling and selected-pane viewport |
| `src/presentation/diagnostics.rs` | Common configuration-error page, source-row cell clipping and diagnostic scrolling |
| `src/presentation/footer.rs` | Effective-binding hint groups, footer row budgets, hint paging and shared search prompts |
| `src/presentation/help.rs` | Help layout, category spacing, selected-entry viewport, wrapped custom bindings and match highlighting; UI state remains in `selection` |
| `src/presentation/themes.rs` | Theme-list picker layout, visible choices and preview highlight; theme state/persistence remain in `selection`/`theme` |
| `src/terminal.rs` | Terminal sizing, raw-mode and Kitty ownership/restoration, stdout drawing and cursor placement |

Dependencies are the existing pinned JSONC parser, `serde_json`, `ratatui`, and
Unicode general-category tables. There is no plugin SDK, async runtime, database,
or network client. Unit tests live under `tests/unit/` and are included by the
library. Direct status-color theme-override cases live in `tests/unit/status_colors.rs`;
mixed startup and configuration-diagnostic assertions stay in their existing suites.
Rendering cases are grouped under `tests/unit/presentation/` into `table.rs`,
`diagnostics.rs`, `footer.rs`, `help.rs` and `themes.rs`, included as children of
`tests/unit/presentation.rs`. The parent retains shared fixtures/text helpers and
cross-view frame/dispatch checks; the children reuse those fixtures rather than
duplicating a harness. Schema, persistence and state-machine checks remain in
the keymap, theme and selection suites, including their mixed rendering assertions. Terminal
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
is a pane ID. Rendering clips by terminal-cell width and sanitizes control text.

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

Select, Help navigation, and Themes resolve their own effective keymaps. Pane
Search and Help search instead have fixed text-editing controls. Subsequence
matching is case-insensitive, per field, and preserves topology/help order.
No CLI call or socket request is caused by a search keystroke. Selection falls
back to the first visible pane when its remembered ID is hidden; empty matches
produce no focus candidate. Navigation is bounded and skips hidden blocks.

The presentation facade resets hint paging, then dispatches configuration errors
before the popup-size guard, Help, the theme list, and finally the pane table
(also used for theme table preview). Each view uses the existing `Ui` state;
there is no separate renderer state machine. The table chooses compact or roomy
columns, maintains its viewport and underlines matches; shared footer hints stay
anchored by each view's layout. The main loop checks interactive size about every
250 ms and pages oversized custom hints every two seconds when needed. Below
80×15 content cells, normal pane actions are blocked; resize can restore the UI.

### Focus and close

Only confirmation in Select with a visible candidate emits a pane ID. `input`
sends `pane.focus` over `HERDR_SOCKET_PATH` and validates reply ID, result type,
exact pane ID, and `focused: true`. Reply reading has a 500-ms overall deadline
and a 16-KiB limit; this is not a general socket-connect/write timeout promise.
Stale/error replies fail without choosing another target. Help, search, themes,
empty results, and error-page close do not focus panes. Terminal ownership restores
the saved `stty` state, shows the cursor, and pops an owned Kitty mode on normal
return or error unwinding.

### Live status and reconnection

The local socket reader feeds a bounded 1,024-message channel. Updates must have
the expected event name, known status, and matching pane/workspace IDs on an
agent row; they cannot rename rows or redirect focus. The loop processes bounded
batches and coalesces redraws. Healthy status streaming does not continuously
refresh topology; reopen to refresh it, or a reconnect installs a fresh snapshot.

On disconnect/failure, background retry is eligible after 500 ms. A worker
resubscribes **before** taking a fresh snapshot and worktree annotations, stages
the result, then the main loop applies queued statuses before installing rows.
The existing UI remains usable during this work. Reconnect CLI calls have a
two-second deadline per command, cancellation, and capped output; subscription
acknowledgment has a 500-ms deadline. Linux x86_64 also has a bounded socket-connect
retry path; other Unix architectures use ordinary connect. Without a nonempty
socket environment variable there is no subscription or reconnect attempt.

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
