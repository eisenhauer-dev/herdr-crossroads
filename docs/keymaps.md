# Keymaps

For setup and the complete defaults overview, see the [README](../README.md).
Developer test layers are described in [CONTRIBUTING](../CONTRIBUTING.md).

Crossroads reads `keymaps.jsonc` once when a popup opens, from the directory
injected as `HERDR_PLUGIN_CONFIG_DIR`. If that file is **absent**, it reads legacy
`keymaps.json` instead. A present but invalid or unreadable `.jsonc` file never
falls back to `.json`. With neither file (or no config directory), defaults apply.
Crossroads never writes, migrates, or deletes keymaps.

## Schema and replacement

The root is an object with optional `select`, `help`, and `themes` objects. Each
action takes an array of **one or two defined key strings**. The limit is checked
before deduplication: even `["q", "q", "q"]` is invalid. An action replaces its entire
default list; omitted actions retain their defaults. There is no disabling an
action, macro, sequence, or configurable search context. Repeated identical keys
within one action are deduplicated, preserving their order. Duplicate field names
at any level are errors, including escaped spellings of the same field name.

JSONC accepts `//` and `/* … */` comments and trailing commas in objects and
arrays, but otherwise requires JSON: double quotes, commas between entries, JSON
numbers, escapes, and whitespace. JSON5 extensions such as unquoted names, single
quotes, hexadecimal numbers, `NaN`, and missing commas are not accepted. The same
syntax applies to the legacy `.json` files.

For example, this replaces pane movement while retaining arrows explicitly:

```jsonc
{
  // Missing actions keep their defaults.
  "select": {
    "next_row": ["J", "Down"],
    "previous_row": ["K", "Up"],
    "open_help": ["?", "!"],
  },
  "help": {
    "next_category": ["J", "Down"],
    "previous_category": ["K", "Up"],
  },
  "themes": {
    "next_theme": ["J", "Down"],
    "previous_theme": ["K", "Up"],
  },
}
```

## Actions and defaults

### `select` — pane navigation

When the popup opens, Crossroads initially highlights the pane focused immediately
before opening it, using `focused_pane_id` from `HERDR_PLUGIN_CONTEXT_JSON`.
The table keeps its topology order and scrolls that row into view without changing
host focus. Missing or invalid launch context, a missing/non-string pane ID, or
an ID absent from the table retains the first-row fallback; an empty table has
no selection. This applies only at startup: later navigation, status updates,
and reconnect snapshots do not reset the highlight to the launch pane.

| Action | Default keys | Effect |
| --- | --- | --- |
| `next_row` | `j`, `Down` | Next visible pane |
| `previous_row` | `k`, `Up` | Previous visible pane |
| `next_tab` | `l`, `Right` | Next visible tab block |
| `previous_tab` | `h`, `Left` | Previous visible tab block |
| `next_workspace` | `Tab` | Next visible workspace block |
| `previous_workspace` | `Shift+Tab` | Previous visible workspace block |
| `open_search` | `/` | Edit the pane search |
| `reset_search` | `r` | Clear the pane filter |
| `open_help` | `?` | Open Keys help |
| `open_themes` | `t` | Open theme selection |
| `focus_pane` | `Enter` | Focus the selected visible pane and close |
| `close` | `Esc`, `q` | Close without focusing a pane |

Pane, tab and workspace movement wraps in both directions through visible rows
or blocks, retaining topology order. Tab/workspace jumps select the first visible
pane of the adjacent block, including when wrapping from any row in the last
block. A single visible pane remains selected; a single block stays the target
block, with jumps selecting its first visible pane. No visible rows means no
movement. Navigation never focuses a pane.

### `help` — Keys navigation, not search input

| Action | Default keys | Effect |
| --- | --- | --- |
| `next_category` | `j`, `Down` | Move forward through visible help entries |
| `previous_category` | `k`, `Up` | Move backward through visible help entries |
| `open_search` | `/` | Edit the independent help search |
| `reset_search` | `r` | Clear the help filter |
| `close` | `Esc`, `q` | Return to the calling Select or Themes view |

`?` opens help from Select or Themes (list or table preview); it does not close Help by default. All close/cancel
lists remain replaceable with one or two custom keys.

### `themes` — list and pane-table preview

| Action | Default keys | Effect |
| --- | --- | --- |
| `next_theme` | `j`, `Down` | Next theme |
| `previous_theme` | `k`, `Up` | Previous theme |
| `toggle_preview` | `Space` | Toggle list/table preview |
| `save` | `Enter` | Persist the choice and return to the table |
| `cancel` | `Esc`, `q` | Discard the preview and return to the table |
| `open_help` | `?` | Open Keys help for Themes without saving or canceling |

Keys help shows only the calling context's actions and effective bindings, not
hard-coded defaults. Its header is `Keys — Select` or `Keys — Themes`; navigation
and search within help use the configured `help` bindings in either case.
Closing help restores the caller, including the exact pane query, selection and
viewport and the theme cursor, list/table preview and saved choice. Opening or
closing help neither focuses a pane nor saves or cancels a pending theme.

Existing search rows show a right-aligned current-mode label: `Select`, `Search`,
`Help` or `Themes`. Query text clips to reserve space for the complete label.
While editing pane or help search, the visible cursor follows the displayed
query (including typed trailing spaces), before padding and the mode label.
Theme table preview shows `Themes`, not `Select`; independent help search still
shows `Help`, separately from its caller header. The theme list has no search
row and identifies itself with its existing Themes heading. Error and undersized
pages have no search-row mode label.

Footers show only the first effective key per displayed action. Navigation
actions additionally show a configured arrow binding if it is not already first,
including custom workspace arrows. No arrow is added to the
actual bindings: replacing an action without arrows removes its default arrows.
Second non-arrow keys and second keys for non-navigation actions appear only in
full Keys help, not footers. Navigation search prompts still show both effective
keys. Footers reserve the required number of rows, up to three, while
retaining space for the table/list and any notice. Default bindings are fully
visible without automatic paging at the supported minimum size of 80×15 and at
wider sizes (for example, two Select footer rows at 80 columns, two at 120,
and one at 240). Custom footers requiring more than that budget page every two
seconds. Oversized selected help entries also scroll automatically; normal help
entry navigation is unchanged. Plain-text footers include all wrapped lines.

Within one action, `/` separates displayed keys, as in `j/↓`. Every footer action
uses `Keybind(s): action`, with direction markers after the colon and ` · `
between all actions in Select, Help, and Themes (list or table preview), for example
`j/↓: ↓pane · k/↑: ↑pane`, `h/←: ↑tab · l/→: ↓tab`, and
`Tab: ↓ws · Shift+Tab: ↑ws`. The tab and workspace arrows describe previous/next
vertical blocks. All displayed arrow key names use `↑`, `↓`, `←`, `→`.
The configuration spelling for a special arrow remains `Down` (or `Up`, `Left`,
`Right`); a collision diagnostic instead uses its display label, such as `↓`.
Typing `"↓"` in configuration is allowed, but binds the literal Unicode character,
not the special Down key, and replaces the action's default list like any other
character binding. Display labels are not configuration syntax. Custom lists use the same format, such as
`J/↓: ↓pane`. Help uses `j/↓: ↓move` and Themes uses `j/↓: ↓theme`;
Help reserves one footer row at 80 columns.
Each `Keybind(s): action` is indivisible, including spaces: a unit
that does not fit moves entirely to the next line. Separators appear only between
actions sharing a line, never at line edges. Footer units are not word/character
wrapped or horizontally scrolled.
Select footers omit `open_search`, `reset_search`, and `open_themes`; Help footers
omit `open_search` and `reset_search`, even with custom bindings. These actions
remain available, and the full Keys help entries, fixed search-editing hints,
and theme picker controls are unchanged.
Keys help retains all effective keys (one or two) for its displayed actions.
A single slash binding is displayed as `/` in Keys help and search prompts,
including the default `Search: / to search`. In multiple-key lists it stays
quoted, for example `'/'/s` or `s/'/'`, to distinguish the key from the separator.
Literal pipe remains displayed as `'|'`. These quotes are display notation only;
footer separator protection is unchanged. In the configuration the key strings
remain `"/"` and `"|"`; key identities, conflicts, and bindings are unchanged.

Keys help filters and highlights the displayed key labels and meanings with the
same case-insensitive subsequence search. Direction markers (`→`, `←`), the `|`
group separator, and quotes around literal separator keys are searchable too,
including in wrapped custom-binding entries. Entries retain their original order.
Help navigation wraps from the first matching entry to the last and vice versa,
with the viewport following selection. No matches leaves selection remembered
without moving; one matching entry remains selected. Themes navigation also wraps
through all offered themes in either view, changing only the unsaved preview.
Configured navigation keys have the same wrapping behavior as the defaults.

## Key syntax

- A single printable character is written directly: `j`, `J`, `0`, `/`, `.`, `,`,
  `-`, `_`, `!`, `"`, `?`, `%`, `&`, `+`, `é`.
  Character bindings are case-sensitive. Space can be `" "` or `"Space"`;
  those are the same identity.
- Character validation uses Unicode 16.0 general categories from the pinned
  `unicode-general-category` 1.1.0 tables. A key must be one Unicode scalar;
  letters, numbers, punctuation, and symbols are allowed. Marks (`Mn`, `Mc`,
  `Me`), Other categories (`Cc`, `Cf`, `Cs`, `Co`, `Cn`), and separators (`Zs`,
  `Zl`, `Zp`) are rejected, except ASCII space (`U+0020`). This excludes standalone
  combining marks, format/zero-width controls, private-use and unassigned scalars,
  and other whitespace. Decomposed characters and multi-scalar emoji sequences
  are not single-character keys. This category policy is not a universal glyph
  visibility guarantee: fonts and terminals can render allowed scalars differently,
  and some allowed letters/symbols may look blank. It validates custom bindings
  only; terminal decoding and search-text handling are unchanged.
- Exact special names: `Enter`, `Esc`, `Backspace`, `Up`, `Down`, `Left`,
  `Right`, `Tab`, `Shift+Tab`, `Space`.
- **No Ctrl or Alt binding is allowed in any context or keyboard protocol**:
  letters, arrows, and modified special keys are all configuration errors.
- Shift-generated printable symbols are stored as characters, for example `!`,
  not physical names such as `Shift+1`. Only `Shift+Tab` is a named Shift binding.
  No F-keys, other modified keys, or sequences are supported.

In both pane and help search, bindings do **not** run. Unmodified printable
characters, including `q` and `?`, become text; Backspace erases. Enter or Esc returns to
the respective navigation mode, retaining the text/filter. Ctrl/Alt input is ignored, including
an entire Alt-letter input. `Ctrl+j` (LF) does not end search and `Ctrl+h` (BS)
does not erase; neither is an artificial alias of Enter (CR) or Backspace (DEL).
In help search, the first Esc ends editing; a subsequent default Esc closes help
and returns to its caller. Help search never changes the pane filter.

## Conflicts and error page

Conflicts are checked per context **after all replacements**, including unchanged
defaults. The same key can be used in different contexts. Giving `next_row` the
key `k` also requires replacing `previous_row` so it no longer owns `k`. One
colliding key in a multiple-key list invalidates the **whole** keymap; there is no
partial application or fallback to defaults.

Empty lists, lists with more than two defined elements, unknown
contexts/actions/keys, invalid structures, duplicate fields,
syntax errors, read errors, and any Ctrl/Alt bindings open a
configuration error page instead of normal operation. Oversized lists are grouped
by mode, for example `ERROR: Maximum 2 keybinds per action - themes mode`,
followed by a plain list of affected actions such as `next_theme`.
The limit heading appears once per mode, with no table or counts.
Validation still counts duplicate and invalid elements before deduplication.
Every error kind has its own heading beginning `ERROR:`, for example
`ERROR: Unknown keymap keys - select mode` or `ERROR: Status colors invalid values`.
There is no umbrella heading or `Other diagnostics` section. Under each heading
is only a simple list of the relevant diagnostics. Distinct error kinds form
stacked blocks separated by horizontal lines. Collisions form one block per mode,
such as `ERROR: Keymap collisions - select mode`: each line lists the
competing actions first, followed by all shared keys for that exact action group,
for example `next_row, previous_row: A/B`. Exact groups merge; partially
overlapping groups stay separate. There is no collision table.
Collision keys use display labels: special arrows appear as glyphs, literal `/`
and `|` are quoted as `'/'` and `'|'`, and shared keys are joined with `/`.
For example, `next_row, previous_row: '/'/↓` is display notation, not a key string
to copy into configuration.
All known-mode keymap diagnostics put the mode **only in the heading**:
`Unknown keymap actions`, `Keymap structure`, `Unknown keymap keys`,
`Duplicate keymap fields`, and `Unsupported keymap keys` each use
` - select mode`, ` - help mode`, or ` - themes mode`, as do limits and collisions.
Lists name the action first where applicable, with no repeated `mode.action`
or `mode:` prefix: `bad_action`, `next_row: expected nonempty key list`,
`next_row: unknown key "F1"`, or `next_row: "Ctrl+i": Ctrl/Alt bindings are not allowed`.
Root, read, syntax, and unknown-mode errors have separate general blocks.
An erroneous unknown-mode name remains in the relevant diagnostic, not in a
heading that pretends it is a supported mode. Duplicate-field scope comes from
the parsed structure; nested action locations and duplicate root names remain useful diagnostics.
Theme and status-color duplicate diagnostics use the same normalized location
style: `duplicate field theme` or `duplicate field working` at the root, and
`extra.nested[0]: duplicate field field` for a nested duplicate, without a `$` prefix.
Every file-related ERROR block has **one source row immediately below its
heading**, before its diagnostic list. It shows the filename in normal text,
then two spaces and the full selected file path in the active theme's dim color.
The full path includes the filename too, for example
`keymaps.jsonc  /home/user/config/keymaps.jsonc`. This applies
to read and syntax errors too: the source is `.jsonc` when present, or legacy
`.json` only when `.jsonc` is absent. A present invalid or unreadable `.jsonc`
never falls back. The same file may have multiple blocks for different error
kinds or modes; each block has its own single source entry. Diagnostic lines
never repeat the configuration path. Paths come from file selection, not from
parsing error text; directly parsed or absent configuration has no invented source.
The source row never wraps. Long paths clip on the right with `…`, using Unicode
cell widths, while preserving the filename where possible. If only the filename
fits, the path is omitted; a clipped path also needs at least one visible path
grapheme before its ellipsis (two cells for a wide first grapheme). The row never
adds two spaces followed by an isolated clipping ellipsis. Popups narrower than
the filename clip it safely too.
Plain-text output uses the same single clipped row, without styling.
The source row scrolls with its block, without path continuation rows.
The popup-size block has no source row. Contexts, actions, key identities,
syntax locations, and reasons remain in the diagnostic lists.
Every visible block heading uses the active theme's bold ERROR notice color; details retain
the normal text color and the theme background is unchanged. Plain-text output
also includes each block heading, without terminal styling or popup-height padding.
Block headings and wrapped details scroll with fixed ↑/k up and ↓/j down, independently of all
configured bindings; Ctrl/Alt+j/k do not scroll. The footer shows
`↑/k: scroll up · ↓/j: scroll down · Enter/Esc/q: close`.
At three or more rows, this footer is anchored at the absolute bottom of the
popup, with a full-popup-width horizontal separator immediately above it.
Neither the footer nor its separator scrolls; short content leaves blank space
above them. Narrow popups clip headings and the footer to fit.
Fixed Enter/Esc/q closes without pane focus, regardless of the faulty bindings.
When interactive content is below 80×15, this common page adds a first block,
`ERROR: Popup too small`, alongside all configuration errors. Its details state
the required and actual content dimensions and advise enlarging and reopening
the popup. For example, 67×12 content shows `need 80x15; actual 67x12.` and
`Enlarge popup and reopen.` Outer popup dimensions are not the content dimensions.
Resizing updates the actual dimensions and removes this block once both minimums
are met; all configuration diagnostics remain scrollable. Noninteractive output
does not add this size block.
Very small popups can only show a portion of the diagnostics at a time; at heights
of one or two rows, affected actions or other diagnostics take
precedence over headings and the close/scroll legend, with a bold ERROR marker
on each visible diagnostic row, starting with the size diagnosis when undersized.
At these heights, block separators and source rows are omitted along with
headings, so diagnostics remain reachable without scrolling through long paths.
At three rows, one content row scrolls through
block headings and diagnostic details above the fixed separator and footer.
An unknown context does not produce redundant unknown-action messages for its
contents; those contents are still checked for invalid structures and keys.

The ordinary too-small page is outside the three contexts: Enter/Esc/q always
closes it without focusing a pane; other keys, including arrows and j/k, are
inert. Its notice starts with `ERROR:` in the active theme's bold error color,
retaining the theme background. It shows the required 80×15 content size,
actual dimensions, and an instruction to enlarge the popup and reopen it.
Plain-text output also includes `ERROR:`. At one or two rows, a compact first
line prioritizes actual and required dimensions; narrower widths clip the text.
Startup collects all independently detectable errors across keymaps,
[theme](themes.md), and [status colors](status-colors.md). Keymap or status-color
errors open this common page instead of aborting; theme errors are included when
that page is open. Theme errors alone retain the normal UI, using Mocha and an
ERROR hint. Unreadable or unparseable files contribute IO or syntax diagnostics,
not invented field errors. Recoverable object fields, including duplicate
occurrences, are all validated. A present invalid `.jsonc` never falls back to
legacy `.json`.

## Terminal limits

On Linux/macOS an interactive popup queries local Kitty keyboard-protocol support.
A valid response within the 100-ms opening probe activates Kitty disambiguation
with its own push; normal closing and error unwinding pop that change. Without
a response, Crossroads uses classical input, **not** a modifyOtherKeys fallback.
An answer arriving after the probe does not switch that popup to Kitty. User
input received before the answer is retained and processed after the probe.
It decodes CSI/SS3 arrows, Ctrl/Alt arrows, and Kitty CSI-u input.

Recognized Ctrl/Alt events are ignored by action resolution and both searches;
their protocol sequences are consumed without leaking into search text.
Configuration rejection is global and does not depend on Kitty detection.
Raw input cannot distinguish physical `Ctrl+i` from Tab (HT) or `Ctrl+m` from
Enter (CR). Prohibiting configuration of the chord does **not** make it possible
to reliably ignore that physical key: HT still navigates as Tab and CR can focus
as Enter in Select. In search, CR still finishes search and HT is ignored.
Kitty-encoded Ctrl identities, when actually delivered distinctly, are ignored.
The current live environment cannot distinguish these physical aliases; no
cross-OS live verification is claimed. Raw BS versus DEL and LF remain unchanged:
BS (`Ctrl+h`) is not a Backspace alias, and LF (`Ctrl+j`) is not an Enter alias.

A raw standalone Esc waits for the fixed **100-ms** ambiguity deadline. A
complete Alt-letter input does not wait out that deadline. Rapid Esc followed by
a letter can therefore be interpreted as Alt in classical input; send Esc alone
and wait when the intention is to return/close.
A second raw Esc within the interval emits the first Esc immediately and starts
a fresh 100-ms interval for the second. If the popup remains open, a subsequent
CSI/SS3 arrow is decoded as an arrow, not inserted as search text.

Herdr 0.9.1/0.9.3 source inspection found semantic popup key routing before
ordinary host bindings and recoding according to the inner PTY mode; its embedded
Ghostty VT supports Kitty query/push/pop. This is **source evidence, not a live
Herdr compatibility test**. An inner Kitty protocol cannot restore identity
already lost at the OS, outer terminal, or host shortcut layer. There is no
universal guarantee for every physical key combination.

Verification sources: [Herdr v0.9.3 input routing](https://github.com/herdrdev/herdr/blob/v0.9.3/src/client/shell/input.rs#L503-L578),
[recoding](https://github.com/herdrdev/herdr/blob/v0.9.3/src/input/encode.rs#L304-L342),
[Ghostty VT](https://github.com/herdrdev/herdr/blob/v0.9.3/vendor/libghostty-vt/src/terminal/stream_terminal.zig#L4702-L4732).

### Windows preparation verification

The Windows WIP uses native console modes and VT input/output, not `stty`.
It saves/restores the original input/output modes, uses native terminal sizing
with an 80×24 fallback, and does **not** send Kitty query/push/pop sequences.
Shared decoding and keymap rules remain in place; this is not a guarantee that
Windows delivers every key identity or resolves the classical aliases above.

The prepared native gate is `python -B scripts/build-windows.py` from the repository
root with [native builder prerequisites](builds.md#windows-preparation-not-certification).
It runs selected Rust suites and `tests/test_windows_acceptance.py` with the
final-linked release executable, compiled fake Herdr CLI, and native ConPTY test
executable selected by the helper. The acceptance harness covers console-mode
restoration, search/help/themes/navigation, resize/close, filtered stable-ID focus,
status disconnect/reconnect/resnapshot, noninteractive EOF/missing CLI and theme
save/reopen/Auto removal. Pipes and CLI are fake-host fixtures, not a real Herdr
server/client. After packaging, the helper also installs in a temporary checkout,
checks exact executable bytes, and repeats acceptance on that installed binary.
The Unix four-PTY commands below are not Windows gates; a plain Windows
`cargo test` does not run this ConPTY suite (its console test is ignored unless
explicitly launched by the harness).

Each acceptance method runs in a separate process with a 30-second deadline,
including cleanup. The suite stops on the first failure. `scripts/windows-process.py`
logs command start/end, PID, exit code and elapsed time; on timeout it attempts
`taskkill /T /F` with a five-second deadline, then kills/reaps the direct child
with a further five-second deadline. Cleanup failures are reported, not ignored.
Captured command output uses temporary files; after command exit or cleanup, the
supervisor snapshots their byte counts and reads at most the first 1 MiB per
stream, reporting cap truncation on stderr even for successful commands. Later
writes are excluded, so a surviving inherited writer cannot prolong capture or
grow its in-memory result without bound. This does not cap temporary-file disk
usage while a command or surviving writer runs. Rust test groups and distribution
checks have 60-second deadlines, each acceptance pass has 240 seconds, and other
helper commands have 300 seconds. The workflow's overall 30-minute limit is unchanged.

The ConPTY fixture keeps its startup pipe ends until `CreateProcessW` completes
and drains output on a separate thread during startup and teardown. Its progress
messages distinguish child exit, `ClosePseudoConsole`, and reader shutdown.
The latter API can block on older Windows; the process deadline, **not** the
reader's five-second join, bounds that call. See Microsoft's
[session lifetime guidance](https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session)
and [shutdown contract](https://learn.microsoft.com/en-us/windows/console/closepseudoconsole)
(consulted 2026-10-21). The native mode test reports the standard-handle value,
console mode and Win32 error on failure; it still requires real console handles
before entering the terminal and verifies exact restoration afterward.

The fixture sets `STARTF_USESTDHANDLES` with null standard handles so Windows
connects them to ConPTY instead of copying redirected parent handles, even with
`bInheritHandles=False`. This addresses the mechanism described by a Microsoft
Terminal maintainer in [discussion 15814](https://github.com/microsoft/terminal/discussions/15814#discussioncomment-6684982)
(2021-09-20; consulted 2026-10-21), alongside Microsoft's
[null-handle attachment contract](https://learn.microsoft.com/en-us/windows/console/getstdhandle#attachdetach-behavior).
An additional native regression explicitly redirects the Python parent's three
standard streams and requires the unchanged mode/restoration test to pass through
ConPTY. No `CONIN$` fallback or plain-input substitution is used.

`Console.until` checks emitted label runs, ignoring only complete SGR styling
sequences, including those fragmented across reads. Cursor/erase commands,
newlines and other controls remain boundaries; control-string payloads are not
labels. OSC ends at BEL or ST (`ESC \`); DCS/SOS/PM/APC remain hidden through
BEL and end only at ST. This is not current-screen reconstruction and does not
join labels across cursor movements or redraws. Protocol-byte checks use `raw=True`.
Returned output and timeout/EOF diagnostics retain the original bytes; output
resets and the five-second default deadline are unchanged. Portable regressions
in `tests/test_windows_support.py` exercise the actual wait loop without invoking
native APIs.

**Native Windows CI has been attempted, but is not green.** The first two runs
reached the 30-minute job timeout. The supplied evidence for
[run 39603155778](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39603155778)
at `343144f` shows failure in 2m 50s: all nine watchdog checks, native Clippy,
compilation and selected Rust suites passed, but initial `GetConsoleMode` still
failed before terminal entry. Rust output bypassed ConPTY; cleanup completed
immediately after the five-second capture timeout, and no artifact was uploaded.
This verified bounded teardown/fail-fast, not console restoration. The supplied
[run 39605428457](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39605428457)
at `bcbd93e` subsequently passed redirected-parent stdio, exact console-mode
restoration, filtered stable-ID focus, live status disconnect/reconnect/resnapshot/
quit, and noninteractive startup/EOF/missing CLI. It failed in 3m 38s at the
search/help/themes/navigation test's `codex` assertion: ConPTY emitted `cod`, SGR
color/underline changes, then `ex`, so the visible label was correct but the raw
substring was absent. Teardown was clean and no preparation artifacts were
uploaded. The SGR-aware matcher above awaits native execution; theme-save
acceptance and the installed-binary second pass were not reached. These passes
prove the handle fix on that runner, not Windows 10/11 certification or overall
pipeline success.
Portable synthetic PE/workflow and supervisor checks are not Windows 10/11
certification. Windows is
preparation for intended 0.3.0, package still 0.2.0. A physical PC is available,
OS version unknown; final actual Herdr popup acceptance is pending **only after
the pipeline stands**, not waived. See [installer/entrypoint limitations](installation.md#windows-preparation-limitations).

## Local verification

Run from the repository root on Linux with Cargo, Python 3.11+ (standard library
only), and the pre-created `/tmp/opencode` directory. These isolated harnesses
use fake Herdr executables and sockets, not real Herdr processes or plugin activation.

`cargo test` covers the keymap, input, action, search, selection, theme, runtime-status
and other unit suites under `tests/unit/`, `tests/configuration.rs`, and startup tests.
The Linux startup tests run the latency/render PTYs and **only the input-protocol
subset** of `pty_keymaps.py` (`--input-protocol`). They do not run the complete
keymap PTY or `pty_hints.py`. The complete keymap and hint PTYs are separate
**mandatory release gates**, not optional substitutes for Cargo tests.

Run all four complete PTY harnesses against a freshly built release binary:

```sh
cargo fmt --check
cargo test
python3 -B -m unittest discover -s tests -p test_pty_readers.py
cargo build --release
python3 tests/pty_latency.py target/release/herdr-crossroads
python3 tests/pty_render.py target/release/herdr-crossroads
python3 tests/pty_keymaps.py target/release/herdr-crossroads
python3 tests/pty_hints.py target/release/herdr-crossroads
```

The Python reader regressions use closed pipes to check immediate EOF failure
before a required marker and termination of frame/tail drains after EOF. Real
PTY children also check post-exit cursor/Kitty restore output, parent slave
ownership through the final drain, and descriptor cleanup on normal exit,
abort, timeout, assertion, and setup failures (including repeated cleanup after
descriptor reuse). Popup exit waits also read pending output, so an intermediate
repaint larger than the PTY queue cannot block the child before exit; a synthetic
child exercises this backpressure in both classic and Kitty modes.
Timing cases send and await complete frames for ordinary context/search preparation
actions separately and assert the final context and query before checking ignored
input or Esc timing. Escape/protocol sequences remain single writes. Complete
Alt-letter and double-Esc arrow checks retain the 95-ms bound, measured through
the matching complete frame without the output-gap reader's extra 10-ms quiet
wait. Alt input must preserve the search screen; the arrow must reach Select with
the expected selected pane, not merely an intermediate navigation frame. Failures
report the measured elapsed time; native scheduling can still exceed this bound.
The popup's draw reader batches ready bytes instead of issuing a readiness check
and read for every byte. It retains read-ahead per popup, splits at complete cursor
markers, and preserves receive timestamps for buffered draws. This avoids charging
thousands of reader system calls to a complete sequence's latency without losing
later frames. Reader regressions check batching and retained frames, reject buffered
early navigation even after delayed processing, and confirm that a modelled 100-ms
response delay still exceeds the unchanged 95-ms fast-input bound. Precise timing
predicates use a deterministic clock and scheduled input, not Python-thread wake-up
times. Real socket/PTY frame separation and lifecycle checks remain separate;
the full native PTY gates still measure actual elapsed time with unchanged bounds.
Long-query render checks await the exact visible search row and editing cursor
in a complete frame within the original three-second deadline, retaining read-ahead
and rejecting intermediate query repaints. Each predicate parses only the latest
full repaint rather than repeatedly replaying the typing history, so fixture
parsing does not grow quadratically. Clipped text does not prove ingestion
of the hidden query suffix.
All standalone search Esc timing checks, including Help search, observe complete
frames rather than treating any readable bytes as resolved input. A delayed
search repaint must retain its entire screen, query and editing cursor; navigation
must retain the query and selection with the cursor hidden. Help must also retain
its contents and return to its keys-navigation footer. The check rejects a
navigation frame whose first byte is received within the original 75-ms guard,
even if its cursor marker arrives later,
and requires a complete transition at 90–220 ms from the Esc write, without
resetting the deadline for intervening frames. Deterministic reader regressions
schedule delayed Search output, including fragmented ANSI and coalesced frames,
and reject early Select/Help navigation (including a 25-ms body with a
105-ms final marker byte) and invalid or missing transitions. The late Kitty reply
check still requires 150 ms without output: no repaint is expected there.
These are receive-time observations,
not internal-state timing or proof of the cause of any particular native macOS failure.

The undersized-popup Enter/Esc/q render cases compare every `termios` attribute
exactly once after successful child exit. On mismatch they report the changed
fields, full saved/active/post-exit attributes, platform and Python version, and
captured output including any immediately ready post-exit `stty` diagnostics.
The active snapshot is taken before sending the close key. Failure-output
collection neither waits nor retries restoration. These diagnostics do not
establish the cause of the Intel installed-binary restoration failure in
[macOS run 39318351418](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39318351418);
native failure output with these details is still needed.

The popup helper and standalone render cases retain the parent's slave descriptor
until final output has been read, then close both descriptors. This avoids a last
slave close discarding unread output: Darwin's
[`ptsclose()`](https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.1.9/bsd/kern/tty_dev.c#L325-L375)
calls [`ttyclose()`](https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.1.9/bsd/kern/tty.c#L376-L465),
which flushes the input and output queues. Output is collected during the bounded
exit wait and then by the post-exit readiness drain, not EOF, while the parent
slave remains open. These regressions run on Linux too; the native macOS
verification below passed on both architectures, including the exit-output checks.

Do not add `--input-protocol` to the complete keymap gate: it returns before
binding-policy, error-page, search, and long-binding checks. The hint gate checks
stable default/custom footers, grouping, and sanitized notices separately. For
theme-table preview it sends and awaits theme opening and Space separately,
asserting list and table content before checking footer stability.

<a id="macos-local-test-preparation-unverified"></a>

### macOS local verification

For preparation on both Apple Silicon and Intel Macs, use a native Rust/Cargo
toolchain with a system linker, Python 3.11+, a POSIX shell, and `stty` on `PATH`.
Run the command block above from the repository root on each architecture;
cross-compiling alone does not exercise native PTYs or sockets. Cargo may fetch
dependencies on a fresh machine; add `--locked --offline` to the Cargo test and
build commands for a network-free run with the locked dependencies already cached.

The harnesses require an existing, owned, writable `/tmp/opencode` directory.
Have the local operator provision it if absent; do not reuse another user's
directory. Keep this literal short path: tests also assert its spelling in
diagnostics. Do not replace it with a resolved alias or a longer system temporary
directory without checking both diagnostic assertions and Unix socket path limits.

The three Cargo PTY wrappers remain Linux-only, so `cargo test` on macOS does
**not** run them. Invoke all four Python commands explicitly, without
`--input-protocol`. The full-listen-queue reconnect test is Linux x86_64-only;
these commands do not provide equivalent coverage on macOS.

At `fa535ba414067bae6df59d6e4394d94149ee0f4b`,
[macOS run 39223794908](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39223794908)
passed native Apple Silicon and Intel checks: formatting, Clippy, Cargo tests,
release build, and all four full PTY harnesses on each architecture. This includes
the harness assertions for `stty` size/raw-state/restoration, PTY reads and closure,
socket paths, and timing. The matching
[Linux run 39223794899](https://github.com/eisenhauer-dev/herdr-crossroads/actions/runs/39223794899)
also passed. The workflow selects `macos-15`/`macos-15-intel` and Rust 1.99.0;
these labels are not exact runner OS-build evidence, and exact OS builds and
Python versions are not recorded here. Record those details for future failures.

The user waived actual Mac Herdr live acceptance on 2026-10-19: **waived, not
passed**. Fake-host PTYs do not verify live popup, navigation, or focus behavior.
The later exact `6f91a3f` Linux and native Apple Silicon/Intel CI also passed all
four full PTYs per platform. These earlier results predate the new 0.2.0
distribution gates; [current three-platform CI and artifact verification](builds.md#current-020-three-platform-ci-evidence)
now cover packaging/provenance and isolated real-Herdr installation.
Released 0.2.0 targets macOS 15.0+ on both architectures, with explicit
`MACOSX_DEPLOYMENT_TARGET=15.0`; published 0.1.0 remains Linux-only. This approved
deployment minimum is not an exact OS-version test matrix or Windows evidence.
Automated installation/distribution is not waived: the new CI gate runs all four
full fake-host PTYs against the real-Herdr-installed binary, without a server or
popup. See [its verified scope and limits](installation.md#automated-release-installation-gate).
Actual Rosetta execution and general macOS compatibility remain unproven.
