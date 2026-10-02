# Native navigation demo provenance

[navigation-demo.gif](navigation-demo.gif) shows a **real isolated Herdr session**,
not the rasterized synthetic CLI/socket fixtures used by the
[overview and theme previews](README.md). Captured on **2026-10-02**, Linux
x86_64, using the real Herdr 0.9.3 server/client in `crossroads-demo`.

## Capture and actions

The approved demo used `/tmp/opencode/crossroads-e2e` as its root and working
directory, with isolated `HOME` in `home/` and `XDG_CONFIG_HOME` in `xdg/`.
Its own `config.toml` selects Catppuccin Mocha with auto-switching and update
checks off. No personal configuration, plugin link, installation command, or
foreign pane was changed. All **24 panes across six workspaces** are actual
shell panes with neutral names, **not running agents or test results**:
Atlas (6), Harbor (6), Beacon (5), Sandbox (5), Codex Lab (1), and Workshop (1).
Welcome, Codex Notes, and Codex Frontend display neutral shell banners; the
remaining panes are idle shells. The host snapshots have an empty agents list.
The initial popup displays 20 pane
rows; the remaining four panes are below the viewport.

The temporary config binds `prefix+f` to the absolute binary
`/home/user/workspace/herdr-crossroads/target/release/herdr-crossroads`, with
`HERDR_PLUGIN_CONFIG_DIR` pointing to the isolated root's `plugin-config/`, and
`width = "90%"`, `height = "90%"`. Ctrl+B then `f` launches it directly.
This is **demo-only**, not a shipped/default binding or standard plugin
registration. Published installation, plugin-link, and plugin-pane-open commands
remain unexecuted examples. Release binary SHA-256:
`75a3e52dfcbdc3975a4b90cd4a5705b771ef4a0d12778c04574761b7741144ea`.

Tokyo Night is already selected in isolated `plugin-config/theme.jsonc` from an
earlier approved capture. That file remains unchanged; **no Themes mode, theme
navigation, or save occurs in this clip**. The final host view still uses Mocha.
The independent [Dracula theme preview](theme-preview.gif) demonstrates saving
with synthetic topology instead.

Nine original PNGs, VTE text exports, host snapshots, and `validation.json` remain
under `/tmp/opencode/crossroads-e2e/capture-long-navigation/`:

| Frame | Actual state / preceding action | GIF hold |
| --- | --- | --- |
| `00-normal-atlas.png` | Atlas → Overview → Welcome, before opening | 2.0 s |
| `01-crossroads.png` | Ctrl+B then `f`: long native pane table | 3.0 s |
| `02-search-empty.png` | `/`: enter search editing | 0.7 s |
| `03-search-o.png` | `o`: query `o` | 0.8 s |
| `04-search-od.png` | `d`: query `od` | 0.8 s |
| `05-search-odx.png` | `x`: query `odx`, four matching panes | 2.2 s |
| `06-results-odx.png` | Enter: leave search, retain filter and Codex Notes selection | 1.5 s |
| `07-frontend-selected.png` | `j`: select Codex Frontend | 2.0 s |
| `08-normal-harbor.png` | Second Enter: focus, popup gone, Harbor → Development split view | 3.0 s |

`odx` matches four panes across three workspaces in topology order: Harbor's
Development panes **Codex Notes** and **Codex Frontend** by pane name, **Codex Lab**
→ General → Session by workspace name, and Workshop → **Codex Tools** → Console
by tab name. The matching fields underline `o`, `d`, and `x`; no actual agent
name match is demonstrated. Enter first leaves editing, `j` moves selection,
and only the second Enter confirms focus.

Focus is independently checked, not inferred solely from the final focused
border: `before-confirm-snapshot.json` reports pane/workspace/tab IDs
`w1:p1`, `w1`, `w1:t1`; `after-confirm-snapshot.json` reports
`w2:p2`, `w2`, `w2:t1`. Both agree with the passed capture `validation.json`;
the final snapshot marks only Codex Frontend focused. Both snapshots retain
24 panes, six workspaces, and no agents.

## Native rendering and GIF fidelity

The capture helper used GTK **3.24.52**, VTE **0.84.0**, and
**DejaVu Sans Mono 12**. Actual VTE allocation was **129×36 cells**; all nine
PNGs are **1299×699 pixels**. These are outer terminal dimensions, not popup
content dimensions. Gdk's `pixbuf_get_from_window` captured only the helper's
own VTE `Gdk.Window`; no desktop or foreign-window capture, ANSI rasterization,
mock CLI/socket, invented chrome, overlays, arrows, cropping, or resizing was
used. Native glyph antialiasing is present in the original PNGs.

Assembly used preinstalled Pillow **12.1.1**, from the repository root:
`python3 /tmp/opencode/crossroads-e2e/assemble-long-navigation.py`.
It reads retained evidence only, without launching Herdr or capturing again.
All nine originals contribute to one shared 256-entry median-cut palette,
applied by exact nearest-RGB-color lookup without dithering. Originals contain
**3,872–6,012 distinct RGB colors per frame**: this is **lossy quantization**,
not exact native-PNG preservation.

Per-frame quantization changes **1.289–3.562% of pixels**; mean absolute
RGB-channel error is **0.0405–0.0964** on the 0–255 scale, with maximum channel
error **30**. Frames use **248–256 palette colors**. Each decoded GIF frame
equals its **quantized RGB raster** byte-for-byte; retained indexed PNGs have
identical palette indices and palettes. This proves GIF encoding against the
indexed exports, not equality with the native originals, which remain untouched.

The GIF is **300,286 bytes**, nine frames, **16 seconds**, loops forever
(`loop=0`), and uses disposal 2. Holds are chosen for readability, **not measured
keypress latency**. GIF SHA-256:
`19ff5eec9bd9c1a8057400fd2dd6730f56b2ff6d99144e01a00d68c6034399c7`.
Per-frame source hashes, errors, durations, decoded equality checks, preserved
asset hashes, and unchanged isolated theme-config hash are recorded in
`/tmp/opencode/crossroads-e2e/long-navigation-media-validation.json`;
indexed exports are in `long-navigation-indexed/` under that root. Earlier
capture and theme evidence remains untouched. These transient files are local
review evidence, not a permanent reproduction package.

## Limits and cleanup

This verifies one native Linux/VTE/Herdr 0.9.3 search/navigation/focus path only.
It does not establish installation/linking, theme saving in this clip,
agent-status updates, reconnect, other platforms, all terminals, or latency
guarantees. No new tools/dependencies or production/test changes were needed for
assembly. `overview.png` and `theme-preview.gif` remain byte-identical.

The owning session stopped its own demo after capture:
`/tmp/opencode/crossroads-e2e/long-stopped-sessions.json` records `crossroads-demo`
as `running: false`. The owner separately checked that the personal Herdr server
remained running on its original socket at 0.9.3. No additional Herdr execution
was needed for assembly or these documentation checks. Independent review and
final acceptance remain separate from this writer's checks.
