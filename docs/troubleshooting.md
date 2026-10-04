# Troubleshooting

Start with the symptom below. Crossroads is navigation-only; do not reset Herdr
configuration, relink other plugins, or delete workspaces to diagnose it.
Key names below are defaults; use your effective [keymaps](keymaps.md) if changed.

## Crossroads does not open: missing binary or host context

The current installation is **build then link a source checkout**, not a prebuilt
binary distribution. The manifest starts `./target/release/herdr-crossroads` from
the linked root; a source-only clone is not ready to launch. There is no manifest
build hook. Keep the linked checkout and executable in place; rebuild after
updating the source. Prebuilt binaries are a future distribution goal, not an
available installation method here.

These read-only checks belong in the **linked checkout root**, in a shell inside
the intended running Herdr session:

```sh
test -x ./target/release/herdr-crossroads && printf 'release executable present\n'
command -v herdr
command -v stty
stty size
```

A failed `test` means absent or nonexecutable, not necessarily a build failure.
`stty size` measures this shell's terminal, **not the popup's content area**.
Crossroads needs a local Herdr CLI (`HERDR_BIN_PATH`, otherwise `herdr` on `PATH`)
and a valid session snapshot. Launching the executable directly outside Herdr
does not supply the plugin context. Snapshot command failures or malformed
responses terminate startup; they are not keymap errors.

Follow [installation](../README.md#installation) to build, link, and open.
Building writes `target/`; linking registers the plugin globally for your user
and creates plugin directories. Neither is a read-only diagnostic. Do not run
install/update/uninstall commands merely to check registration.

## The open command returns, but no popup is visible

There is no established Crossroads-specific cause for this symptom. Record the
host's exact error, if any; check the intended session, registration, linked root,
and executable before changing anything. `herdr plugin list` is the host's
registration inspection example. If the host reports `ui_busy`, close the other
modal before trying again. The README's open command requests 90% width/height;
the manifest itself supplies no dimensions and the host default is half-size.

Host commands here and in the README were **statically checked against Herdr
v0.9.3 sources, not executed in a live session**. Opening changes the host UI;
it is not a read-only probe. Do not infer that a missing popup is caused by size,
theme, or terminal protocol without evidence. Include versions, the command,
error text, and content dimensions in a report; omit private topology/config data.

## “Crossroads needs minimum content size 80x15” / “Popup too small”

The minimum is **80 columns × 15 rows of content**, excluding host borders.
Below either dimension, pane focus is disabled. Enlarge the outer terminal and
request a larger popup, then reopen if needed; 90% of a small terminal may still
be too small. Resize is detected while open. Configuration errors can share the
error page with a size block; fixing size does not fix those errors.

On the size-only page, Enter/Esc/`q` closes without focus. On the common error
page, Up/`k` and Down/`j` scroll; Enter/Esc/`q` closes without focus. These keys
are fixed even when the configured keymap is invalid.

## A configuration error page appears, or an edit seems ignored

Crossroads reads plugin files **once per opening** under `HERDR_PLUGIN_CONFIG_DIR`.
Reopen after edits. For each basename, `.jsonc` wins; legacy `.json` is read only
when `.jsonc` is absent, not when it is unreadable or invalid. A dangling `.jsonc`
symlink is not absence. Without the injected directory, defaults apply.

`herdr plugin config-dir herdr-crossroads` is an unexecuted host example for
finding the directory; **it may create that directory**. Do not use it as an
implicitly read-only check. Inspect only your own selected files; do not dump
the whole environment or unrelated configuration into a report.

Read the error kind and source row, then scroll for remaining diagnostics:

| Symptom | What to check |
| --- | --- |
| Syntax / duplicate fields | JSON with comments/trailing commas is accepted; single quotes, unquoted fields, other JSON5 syntax, and duplicate fields are not. |
| Unknown mode/action or structure | Only `select`, `help`, `themes` and the [documented actions](keymaps.md#actions-and-defaults) exist; each action needs one or two key strings, not an empty list. |
| Maximum 2 keybinds | Limit is checked before deduplication; even three identical entries fail. |
| Keymap collisions | Replacements conflict with unchanged defaults too. Assigning `next_row` to `k` requires moving `previous_row` off `k` in the same mode. No partial keymap is applied. |
| Unsupported / unknown key | Use a case-sensitive single allowed Unicode letter/number/punctuation/symbol, ASCII space, or an exact special name below. No Ctrl/Alt, F-keys, sequences, or modified special keys except `Shift+Tab`. |
| Status colors invalid values | Only `working`, `blocked`, `done`, `idle`, `unknown`, each a `#RRGGBB` string. |

Special names are `Enter`, `Esc`, `Backspace`, `Up`, `Down`, `Left`, `Right`,
`Tab`, `Shift+Tab`, and `Space`. Write `!`, not `Shift+1`; combining marks,
format/control characters, private-use/unassigned characters, and non-ASCII
whitespace are not allowed. See [key syntax](keymaps.md#key-syntax) for full rules.
Collision glyphs such as `↓` are display labels: configuring `"↓"` binds that
literal character, not the special `"Down"` key.

Keymap/status-color errors block the normal UI. Theme errors alone instead use
Mocha and a notice; if combined with those other errors they join the common
page. Crossroads never writes keymaps or status colors. Back up your file before
manually correcting it; do not delete unrelated plugin or host configuration.

## No matching panes, or `r` / `q` acts like text

Search is case-insensitive **subsequence**, not substring or ranked fuzzy search.
It matches one workspace, tab, pane, or agent field at a time, never across
fields; status text is not searchable. Results stay in topology order.

While editing search, printable keys including `r`, `q`, `t`, and `?` are text.
Backspace erases; Enter/Esc leaves editing **and retains the filter**. Then `r`
resets it in navigation mode. Enter with no matches focuses nothing. If the
remembered pane is hidden, the first visible pane becomes the focus candidate.
Keys help has an independent search; resetting it does not reset pane search.
See the [captured demonstration](../README.md#preview).

## Enter fails to focus a pane / the selected pane disappeared

In search editing, Enter only returns to navigation. In navigation, focus uses
the selected **pane ID**, not its displayed name. A pane closed since the snapshot
can be stale. Crossroads does not retry another pane on failure. Reopen for a new
snapshot and select again; never assume a same-named row is the same pane.

Focus requires the local `HERDR_SOCKET_PATH`. The following read-only shell check
reveals only whether a socket path was supplied and is currently a socket:

```sh
if [ -z "${HERDR_SOCKET_PATH:-}" ]; then
  printf 'No Herdr socket path supplied\n'
elif [ -S "$HERDR_SOCKET_PATH" ]; then
  printf 'Socket file present; connection and focus are not yet verified\n'
else
  printf 'Supplied path is not a socket file\n'
fi
```

A socket file is **not** proof of a reachable/healthy session, and a shell may
not have precisely the popup's injected environment. Missing socket context
still permits browsing the CLI snapshot, but not focus or live status. Reopen
from the intended host session instead of inventing or copying a socket path.
Rejected, malformed, mismatched, or timed-out focus replies cause failure, not a
successful focus demonstration. Do not test focus on a live pane without intending
to change host focus.

## Statuses are stale, absent, or do not change

Without a nonempty socket path, live status is disabled. Otherwise Crossroads
subscribes to pane agent-status events and retries failed/disconnected
subscriptions in the background. It keeps the current rows while disconnected;
there is no connection-health indicator, so a visible status is not proof of an
active subscription. A successful reconnect obtains a fresh snapshot. Search
itself never re-queries Herdr, and the subscription is not a general continuous
topology refresh; reopen after workspace/tab/pane changes when needed.

Only matching pane/workspace events for rows with an agent and recognized statuses
update rows. Panes without agents have no status; unrecognized snapshot statuses
use the unknown color. At less than 110 content columns, statuses are inline;
at 110+ they have a separate column. Custom [status colors](status-colors.md)
override theme colors, so unexpected colors may be intentional configuration.

## Theme falls back to Mocha, changes are not saved, or Auto looks different

Auto conservatively recognizes a simple manual built-in Herdr theme. SSH context,
missing/unreadable config, custom settings, ambiguous syntax, and auto-switching
fall back to Mocha; Auto does not follow live host theme changes. Config path
precedence is `HERDR_CONFIG_PATH`, then `$XDG_CONFIG_HOME/herdr/config.toml`, then
`$HOME/.config/herdr/config.toml`—not retry-after-read-failure fallbacks.

`theme.jsonc` accepts exactly one `theme` field with an explicit offered built-in
name; `Auto (Herdr)` is a picker entry, not a file value. Invalid/unreadable theme
files are untouched and produce a Mocha/error notice. `terminal` intentionally
uses terminal default colors; other themes paint their own panel.

Preview is temporary: Enter in Themes **saves**, Esc/`q` cancels. Saving an explicit
theme atomically replaces plugin-owned `theme.jsonc` without preserving comments;
explicitly saving Auto removes **both** theme files. This is a deliberate file
mutation, not a diagnostic. A failed save/delete keeps the picker open with an
error and does not claim success; Auto deletion can already have removed one file.
Check the plugin directory's writability without changing permissions blindly.
See [Themes](themes.md) for exact recognition and persistence behavior.

## Ctrl/Alt keys do nothing, or Esc seems delayed

Ctrl/Alt bindings are prohibited in every mode. Recognized modified input is
ignored, not a hidden navigation binding. Classic terminal bytes cannot
distinguish physical Ctrl+i from Tab or Ctrl+m from Enter; do not rely on those
physical chords being rejected as distinct keys. Conversely Ctrl+j (LF) is not
Enter (CR), and Ctrl+h (BS) is not Backspace (DEL).

Standalone raw Esc has a 100-ms ambiguity delay. Rapid Esc followed by a letter
may be Alt input, not two actions; use Enter to leave search or allow Esc to
resolve before the next key. Enhanced keyboard support depends on the terminal
and forwarding host. See [terminal limits](keymaps.md#terminal-limits); isolated
classic/Kitty tests do not verify every live outer-terminal/Herdr combination.

## Evidence and example status

Behavior above follows `src/runtime.rs` (startup/input loop),
`src/runtime/live_status.rs` (status/reconnect), `src/topology.rs` (snapshot),
`src/selection.rs` and its `help`/`themes` children (state/input), `src/search.rs`
(matching), `src/input.rs` (decoding), `src/actions.rs` and `src/focus.rs`
(action execution/focus), `src/config.rs`, `src/keymap.rs`, the `palette`,
`discovery` and `persistence` children of `src/theme.rs`, and `src/status_colors.rs`.
`src/presentation.rs` dispatches views and paints frames; its `diagnostics`,
`table`, `footer`, `help` and `themes` child modules own their respective layouts.
Rendering checks live under `tests/unit/presentation/`, with shared and cross-view
checks in `tests/unit/presentation.rs`. Selection and theme checks mirror their
domain modules under `tests/unit/selection/` and `tests/unit/theme/`; input, action,
search and runtime-status cases have dedicated unit suites. Mixed keymap
configuration/rendering assertions remain in `tests/unit/keymap.rs`.
Isolated unit/integration and PTY coverage lives under
`tests/`. Host-command lifecycle
claims use the versioned sources linked in the [README](../README.md#installation).
Shell diagnostic snippets are read-only and statically checked here, **not run
against a user's live installation**. The separate [native demo](media/navigation-demo.md)
opened a custom binary-launch popup with preselected Tokyo Night, searched a
24-pane list, and verified focus in an isolated Herdr 0.9.3 session; it did not
open Themes or save a theme. It did not exercise this
guide's installation/plugin-link commands, reconnect, or host configuration repair.
Report exact errors and versions, not private snapshot payloads.
