# Media provenance

The [native navigation demo](navigation-demo.gif) captures a **real isolated
Herdr 0.9.3 session**: 24 shell panes across six workspaces, `/odx` matching four
panes by pane/workspace/tab fields, and confirmed focus after popup dismissal.
Tokyo Night is preselected; no theme picker or save is included. It is
1299×699 px, nine frames, 16 seconds, and 300,286 bytes. See its
[capture actions, focus evidence, cleanup, and quantization limits](navigation-demo.md).

The two independent assets below are **actual Crossroads output with synthetic
demo data; not live Herdr screenshots**. No real session topology, personal
names, or workspace paths were captured for these assets. They have no invented
host window chrome or success notification. The capture/rendering details below
apply only to these two synthetic assets, which remain byte-identical.

| Asset | Captured state | Dimensions / size |
| --- | --- | --- |
| `overview.png` | Initial pane table, three synthetic workspaces, eight panes | 1100×440 px; 7,351 bytes |
| `theme-preview.gif` | Mocha table → `t` theme list → `j` through Mocha, Latte, terminal, Tokyo Night, Tokyo Night Day, Dracula → Space table preview → Space list → Enter saves → normal Dracula table | 1100×440 px; 70,695 bytes |

## Synthetic capture

Captured on 2026-10-02, Linux x86_64, from the current implementation (base
commit `6425d80`, with documentation-only working changes). Build from the
repository root: `cargo build --release --locked --offline`. Rust/Cargo 1.93.1;
release binary SHA-256:
`75a3e52dfcbdc3975a4b90cd4a5705b771ef4a0d12778c04574761b7741144ea`.

The temporary helper `/tmp/opencode/crossroads-media.py` produced the overview;
its synthetic snapshot, CLI/socket logs, and fourteen ANSI/text frames with
rendered PNGs remain under `/tmp/opencode/crossroads-media/`. That original
capture also previewed and canceled themes; it is historical evidence, not the
current theme GIF. The saved-theme capture used
`python3 /tmp/opencode/crossroads-themes.py` from the repository root. Its
separate snapshot, logs, eleven ANSI/text/PNG frames, `validation.json`, and
runtime-written `plugin-config/theme.jsonc` remain under
`/tmp/opencode/crossroads-media/themes/`. Neither original directory was
overwritten. These helpers are not shipped tools or repository dependencies;
their temporary files are not a permanent reproduction package.

The overview helper reused `read_frame` from `tests/pty_keymaps.py` and `screen`
from `tests/pty_render.py` without changing either. The theme helper reused the
transient ANSI renderer with its own text-only screen parser and PTY read loop;
it did not read/import project source or tests. Both set a Linux PTY to
**110×20 content cells** with `TIOCSWINSZ`. An isolated fake CLI answered
`api snapshot`, `pane list`, and empty `worktree list` calls with neutral
synthetic topology (Atlas, Beacon, Sandbox; eight panes). A temporary Unix
socket acknowledged the real binary's `events.subscribe` request and held the
connection open, without fabricated status changes. Snapshot statuses are
synthetic examples, not measured agent activity. These captures show no live
pane focus; they use a subscription-only socket.

`HOME`, XDG config, host theme path, and plugin config were isolated; own-pane
context was removed, selecting default keymaps and Mocha fallback. Config
started empty. The original canceled-theme run ended empty; the saved-theme
run ended with exactly `theme.jsonc`, asserted as `{"theme":"dracula"}`,
written by the actual release binary on Enter, not preseeded by the helper.
No personal config was used or changed.

The theme loop starts at the normal Mocha table; `t` opens Themes at Auto
(Herdr). Six separately captured `j` keys move through `catppuccin`,
`catppuccin-latte`, `terminal`, `tokyo-night`, `tokyo-night-day`, and `dracula`.
Each frame retains the list and changing preview name; the selected/saved
header remains Auto until saving. Space shows the Dracula table preview;
Space returns to the list; Enter saves and closes Themes. The normal Dracula
table is held for 3.2 seconds, then `q` exits without focus. Assertions check
no writes during navigation, exact saved JSON, unchanged pane order/selection
and empty filter, and final text/background/selection equality with the picked
theme and table preview. Final text/background are `#f8f8f2` / `#282a36`;
selected-row background is `#463f5d`. The terminal preview inherits renderer
defaults, not a portable fixed terminal palette.

## Synthetic rendering fidelity and limits

Python 3.14.4 and preinstalled Pillow 12.1.1 rasterized the ANSI cells. The
renderer supports only captured cursor positioning, erase display/line, SGR
RGB/indexed colors, bold/dim/underline, reverse-video selection, and cursor
visibility; unsupported operations abort. Cell text was compared independently
against the PTY screen parser for overview and the text-only parser for theme
frames. Colors, selection extent, spacing, and cell positions come from output,
not a hand-composed UI.

Font: system DejaVu Sans Mono Book/Bold, 16 px, fixed **10×22 px cells**, with
binary glyph masks (no antialiasing). Book font SHA-256:
`a54dca07c76d6289e717e75e0a58c0128f6d7269ef3faf76417c9d7d3bba37ab`.
These are rasterized PTY captures, not screenshots of a particular terminal.
Glyph metrics, underline position, dim intensity, and steady underline cursor
are renderer choices where ANSI leaves behavior unspecified. Labels use
single-cell characters; no general wide/combining-glyph claim is made. No outer
terminal chrome, blinking, host borders, or keyboard-protocol response is shown.

The eleven theme GIF frames use an exact shared **26-color palette**; explicit
RGB-to-index mapping avoids approximate quantization, with no dithering. Every
decoded frame equals its captured raster byte-for-byte. The GIF loops forever
(`loop=0`, disposal 2); holds in milliseconds are
`2000, 1600, 1800, 1800, 1600, 1800, 1800, 2200, 1800, 1400, 3200`
(21 seconds). Timing is chosen for readability, **not measured input latency**.
Dimensions, frame count, duration, palette fidelity, and size were checked;
theme list/final-table rasters were visually inspected by the capturing session.

Preserved SHA-256 hashes:

- `overview.png`: `9e522747fa73e8a215887c5b314b35dbf5477c92415e4dbc10c7f4ad8ce9959c`
- `theme-preview.gif`: `60892a133abf93d22d8dc6e556253d727a58157124e7a92686f8384d8f9055f2`

No new tools/dependencies, network access, live Herdr changes, or production/test
modifications were needed for these two synthetic captures or their preservation
checks. The native recording has separate provenance above.
