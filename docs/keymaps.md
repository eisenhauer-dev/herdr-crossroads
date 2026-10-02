# Keymaps

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

### `help` — Keys navigation, not search input

| Action | Default keys | Effect |
| --- | --- | --- |
| `next_category` | `j`, `Down` | Move forward through visible help entries |
| `previous_category` | `k`, `Up` | Move backward through visible help entries |
| `open_search` | `/` | Edit the independent help search |
| `reset_search` | `r` | Clear the help filter |
| `close` | `Esc`, `q` | Return to the pane table |

`?` opens help from Select; it no longer closes Help by default. All close/cancel
lists remain replaceable with one or two custom keys.

### `themes` — list and pane-table preview

| Action | Default keys | Effect |
| --- | --- | --- |
| `next_theme` | `j`, `Down` | Next theme |
| `previous_theme` | `k`, `Up` | Previous theme |
| `toggle_preview` | `Space` | Toggle list/table preview |
| `save` | `Enter` | Persist the choice and return to the table |
| `cancel` | `Esc`, `q` | Discard the preview and return to the table |

Keys help shows the effective pane-navigation lists, not hard-coded defaults.
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

## Key syntax

- A single printable character is written directly: `j`, `J`, `0`, `/`, `.`, `,`,
  `-`, `_`, `!`, `"`, `?`, `%`, `&`, `+`, `é`.
  Character bindings are case-sensitive. Space can be `" "` or `"Space"`;
  those are the same identity.
- Exact special names: `Enter`, `Esc`, `Backspace`, `Up`, `Down`, `Left`,
  `Right`, `Tab`, `Shift+Tab`, `Space`.
- **No Ctrl or Alt binding is allowed in any context or keyboard protocol**:
  letters, arrows, and modified special keys are all configuration errors.
- Shift-generated printable symbols are stored as characters, for example `!`,
  not physical names such as `Shift+1`. Only `Shift+Tab` is a named Shift binding.
  No F-keys, other modified keys, or sequences are supported.

In both pane and help search, bindings do **not** run. Unmodified printable
characters, including `q`, become text; Backspace erases. Enter or Esc returns to
the respective navigation mode, retaining the text/filter. Ctrl/Alt input is ignored, including
an entire Alt-letter input. `Ctrl+j` (LF) does not end search and `Ctrl+h` (BS)
does not erase; neither is an artificial alias of Enter (CR) or Backspace (DEL).

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

In an interactive popup Crossroads queries local Kitty keyboard-protocol support.
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
Local coverage is in `tests/unit/keymap.rs`, `tests/a10c.rs`, and the isolated
`tests/pty_keymaps.py`; no real Herdr processes are used by those tests.
