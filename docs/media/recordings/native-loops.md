# Native navigation and theme loops

Recorded native Crossroads UI, with editorial captions **below** the original
1656×812 image. Both silent GIFs are 1656×896, sampled at 12 fps, and loop forever.
No UI pixels are painted, scaled or replaced before GIF palette quantization.
The first caption discloses the off-screen reset at each loop seam.

| Asset | Duration | Frames | Bytes |
| --- | ---: | ---: | ---: |
| [Navigation](../native-navigation.gif) | 19.75 s | 237 | 1,070,492 |
| [Themes](../native-themes.gif) | 12.00 s | 144 | 971,384 |

## Navigation

The entire 16.48-second source is slowed uniformly by 1.2, then sampled at 12 fps.
Timing is editorial, not a latency measurement. Phase boundaries come from
decoded selection rows and the visible Search/Select badge, not tape sleeps.

| Source start (s) | Caption/action |
| ---: | --- |
| 0.00 | Reset off-screen; start on Welcome |
| 1.88 | `j`, then `k`: select panes |
| 3.60 | `l`, then `h`: select tab blocks |
| 5.16 | Tab, then Shift+Tab: select workspace blocks |
| 6.88 | `/`, then `odx`: live fuzzy search with editing cursor |
| 10.12 | First Enter: finish search, keep the filter; badge becomes Select |
| 12.00 | `j`: select Codex Frontend in the filtered table |
| 13.80 | Second Enter: close the popup and focus Codex Frontend |

Source snapshot evidence confirms focus changes from Welcome (`w3:p1`) to
Codex Frontend (`w5:p2`); all 76 snapshots retain six workspaces, seven tabs and
28 panes. Navigation before the final Enter does not focus another pane.

## Themes

Chronological **stable decoded frames are held**, rather than replaying elapsed
capture time. The earlier Select Help search and nested Themes Help detours are
omitted, as are transient incomplete palette repaints. No sustained rendering
fault or responsiveness conclusion is inferred from those omitted frames.

| Source frame time (s) | Output hold (s) | Native state/action |
| ---: | ---: | --- |
| 9.50 | 1.50 | `t` has opened Auto theme list; reset off-screen |
| 15.30 | 0.75 | `j`: Catppuccin preview |
| 15.70 | 0.75 | `j`: Catppuccin Latte preview |
| 16.00 | 0.75 | `j`: Terminal preview |
| 16.30 | 0.75 | `j`: Tokyo Night preview |
| 16.70 | 0.75 | `j`: Tokyo Night Day preview |
| 17.00 | 1.25 | `j`: Dracula preview |
| 18.50 | 2.00 | Space: Dracula pane-table preview; Welcome still selected |
| 22.50 | 1.50 | Space: return to Dracula list |
| 24.50 | 2.00 | Enter: save Dracula and return to Select; Welcome unchanged |

The captions show the available `j/k` controls; this capture steps forward with
`j`. All 117 source snapshots keep focus on Welcome and retain the same topology.
Source evidence records no theme file before and `{"theme":"dracula"}` in
`theme.jsonc` afterward: save/persistence proof, not a cancellation demonstration
or a recorded reopening. Behavior references: [keymaps](../../keymaps.md),
[themes](../../themes.md), `src/presentation/footer.rs`,
`src/selection/themes.rs` and `tests/unit/presentation.rs`.

## Provenance and verification

Rendered locally with FFmpeg 9.0.2 and existing Pillow 12.1.1 (`/usr/bin/python3`).
Captions are Pillow PNG bands; FFmpeg uses RGB overlay, lossless FFV1 intermediates,
then `palettegen=stats_mode=full` and
`paletteuse=dither=bayer:bayer_scale=3`, with `-an -loop 0`.
All outputs use `-n`; existing sources and older assets are preserved.

SHA-256:

```text
navigation-search-cursor.mp4  0a3eb274b516a9d4198f9e23d078e84dd41ebebfc6534d34a23a0756de5cb643
help-themes-cursor.mp4        74146ff8587a8894fc8e3fffc4111a46f23e77b81aad3ee319a2cfc308c2e2a6
native-navigation.gif        30f171eedee49567c8828865d762748450c6f2631db75f70f2019c51d801816a
native-themes.gif            cf4aa5d6c779b39261a8c07f105149de0d5e1b348260fbec3bc680d150fbfbd5
captured Crossroads binary   58f12f014d9c78c3f5ebc92e4c8867c1bbb71dc456d83eef49a78303b66bd2ec
```

Local source recordings and corresponding `*-evidence.json` are under
`/tmp/opencode/xrd/`. Local, non-shipped render evidence is under
`/tmp/opencode/xrd/finishing/native-loops/`:

- `commands.json`: every exact executed FFmpeg render command; `command-*.log`:
  command results. `render.py` records caption construction; `themes.ffconcat`
  records holds. These are local provenance, not installed project helpers.
- `phases.json`: source/output caption boundaries and theme RGB frame hashes.
  `navigation-transitions.json`: decoded selection/badge/query transitions.
- `validation-commands.json` and `validation.json`: executed checks, source and
  output hashes, FFprobe metadata, caption checks and every GIF frame delay.
- `navigation-*.framemd5`: all 237 pre-palette native RGB crops exactly match
  the slowed/sampled source. All ten theme frames were checked for exact native
  RGB preservation before quantization.
- `*-output-phase-*.png`, `*-output-crop-*.png` and output contact sheets:
  representative decoded GIF frames inspected for native states and captions.

Checks ran from `/home/user/workspace/herdr-crossroads`:

```sh
/usr/bin/python3 /tmp/opencode/xrd/finishing/native-loops/render.py
/usr/bin/python3 /tmp/opencode/xrd/finishing/native-loops/validate.py
```

Validation passed: FFprobe dimensions/duration and no audio stream; full FFmpeg
decode with `-xerror`; Pillow decoding of every frame, loop value `0`, summed
frame delays and all caption phases; unchanged source hashes and preserved native
RGB crops. The local scripts require the existing evidence inputs and fresh
output paths; they are executed provenance, not a portable recreation recipe.
No capture, application change or software/PTY test rerun belongs to this render
package. Combined review and final acceptance remain pending.
