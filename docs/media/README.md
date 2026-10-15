# Media provenance

## Current movie acceptance

The current [guided demo](crossroads-circular-tour.mp4), SHA-256
`5497401d6dc418ae6bb0d7069fd397f90a2dd60d9447728bc5d86940ad4ed054`,
has passed independent final-film review and received final user acceptance on
2026-10-13. The user has also explicitly accepted the current movie for privacy.
These are supplied review/acceptance decisions, not a fresh movie review by this
documentation writer. They include the closing-credit removal, not just the
preceding cut. See [current export provenance](recordings/guided-tour.md).
Publication authorization remains separate; no publication was performed here.
Missing original external captures and evidence remain missing; acceptance does
not recreate native capture, topology or persistence proof.

## Narration use and attribution

The user confirms that **all narration used in the movie was generated on the
ElevenLabs Free plan**, and that the planned use is noncommercial: no monetization
or marketing of commercial goods or services. Free/open-source software is not,
by itself, evidence that a particular use of its demo is noncommercial.

Primary sources checked on **2026-10-15**:

- [Publication guidance](https://help.elevenlabs.io/hc/en-us/articles/13313564601361-Can-I-publish-the-content-I-generate-on-the-platform)
  says Free output has no commercial license and requires `elevenlabs.io` or
  `11.ai` **in the title** when shared noncommercially. It also says output made
  outside a paid subscription cannot be used commercially; a later upgrade does
  not establish commercial rights to these existing Free generations.
- Both [EEA/Switzerland/UK terms](https://elevenlabs.io/terms-of-use-eu) and
  [non-EEA terms](https://elevenlabs.io/terms-of-use), last updated **2026-10-09**,
  restrict Free use to noncommercial purposes in §1(c), require a personal email
  address for noncommercial use, and preserve content rights subject to the terms
  in §4(a), §4(c) and §4(g). Applicable terms depend on the account holder's
  residence, not the repository location. The EEA version includes country-specific
  amendment/notice rules, including Germany; fetching today's terms does not prove
  which revision the account holder accepted for an earlier generation. Residence,
  account email and contract notices were not inspected here.
- [Service-Specific Terms](https://elevenlabs.io/service-specific-terms) and
  [ElevenAPI Terms](https://elevenlabs.io/elevenapi-terms), both dated **2026-10-09**,
  retain applicable service/model terms; API use is not an exception to the Free
  restriction. [Beta Services Addendum](https://elevenlabs.io/bsa), updated
  **2024-11-13**, §3 forbids commercial purposes and production-environment use
  of Beta Services, including on paid plans.
- The historical generation record identifies Brian, premade voice ID
  `nPczCjzI2devNBz1zQrb`, and `eleven_multilingual_v2`, not a custom voice clone.
  The [current model documentation](https://elevenlabs.io/docs/overview/models#multilingual-v2)
  lists Multilingual v2 as a regular previous-generation TTS model, without a beta
  designation. That supports the recorded model choice, but is not account-level
  proof of generation-time entitlements. The [Voice Library Addendum](https://elevenlabs.io/vla),
  dated **2026-03-06**, distinguishes provider voices from user voices; terms
  §4(g) still requires necessary rights to inputs and outputs. No new voice lookup,
  account access or generation was performed, and no blanket voice-rights guarantee
  is made.

The README video heading retains **Crossroads Demo - voiceover by elevenlabs.io**;
the local MP4 title metadata is **Crossroads Demo — voiceover by elevenlabs.io**.
Both contain the required title credit. Keep that credit in the actual title of
any separately authorized upload or redistribution; metadata alone is not a
substitute for the hosting platform's visible title. The remote README attachment
was not downloaded or hash-verified during this check.

The software remains MIT-licensed. That license must not be read as a grant of
commercial reuse rights to the Free-generated narration. This records provider
conditions, not a new media license or legal guarantee. Commercial reuse of this
narration is outside the confirmed scope and needs separate rights clearance;
recheck terms if the use, model, voice or distribution context changes.

## Retained image privacy inspection — 2026-10-15

All frames of the five GIFs were decoded with existing Pillow. Exact RGB duplicates
were deduplicated by SHA-256; every distinct decoded frame was visually inspected
in labeled half-size contact sheets, and `overview.png` was inspected directly.
The inspection covered the host UI, popup, pane labels, shell text and captions,
not just file metadata or representative animation timestamps.

| Asset | Decoded frames | Distinct RGB frames inspected |
| --- | ---: | ---: |
| `linked-worktrees.gif` | 168 | 69 |
| `native-navigation.gif` | 237 | 178 |
| `native-themes.gif` | 144 | 10 |
| `navigation-demo.gif` | 9 | 9 |
| `theme-preview.gif` | 11 | 10 |
| `overview.png` | 1 | 1 |

No apparent personal names, email addresses, personal home paths, credentials or
private workload content were found in the readable visuals. Visible names and
branches are neutral demo context; synthetic previews include example agent names
and statuses. This is a visual assessment, not OCR, hidden-metadata forensics or a
guarantee that no sensitive data exists. Contact sheets are local inspection aids
under `/tmp/opencode/herdr-crossroads-media-privacy/inspection-2026-10-15/`, not a
permanent reproduction package. No image or movie bytes were changed. This image
inspection does not recreate the unavailable original capture evidence or replace
the user's separate movie acceptance. Git author/committer names and emails were
explicitly accepted for public visibility by the user; that decision is separate
from inspection of image content.

## Earlier native and synthetic previews

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
