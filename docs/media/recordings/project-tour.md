# Project tour: native capture and editorial validation

`crossroads-project-tour.mp4` was created from fresh native project takes and the
existing narration, then superseded by the [circular-navigation tour](circular-tour.md)
and removed at the user's request. Its source record remains relevant to the
retained project introduction. The checks below describe the historical render
before authorized cleanup, not current export availability or release approval.
**Technical media validation passed; independent revised-media acceptance, human
listening and publication authorization remain pending.** No new narration, dependency,
network request, agent activity or capture was performed by the compositor.

## Final media

- H.264 video and mono AAC audio only: exactly two streams, no subtitle stream.
- 1656×812, 25 fps, **3,789 video frames / 151.56 seconds of content**;
  container/audio duration **151.583220 seconds**, 5,227,933 bytes.
- No burned-in subtitles, editorial key captions or bottom editorial band.
  Complete native terminal frames are uncropped and fill the image during native
  scenes; native key footers remain part of the actual application.
- SHA-256: `865af44d4c79e36078fcd788291122e5aeb6e6743423e0cbf8d6a9ca9a4287f2`.
- Title and retained closing-card attribution:
  **Crossroads Demo — voiceover by elevenlabs.io**. Terms and permitted publication
  use still require a current check; this artifact is not publication approval.

## Actual project dataset

The capture owner renamed the existing isolated Herdr topology: **six workspaces,
seven tabs, 28 ordinary shell panes, zero agents**. These realistic names are
labels and neutral static shell banners, not evidence that editors, package
scripts, servers, tests, deployments or containers are running. No native process
output is simulated. The diagram and terminal use the same relevant names.

| Workspace | Tab | Panes |
| --- | --- | --- |
| Atlas Shop | Frontend | Editor, pnpm dev, Storybook, git diff |
| Atlas Shop | Tests | Unit Tests, Integration Tests, Type Check, Lint |
| Atlas Checkout (linked to Atlas Shop) | Checkout | Checkout Editor, pnpm dev, Checkout Tests, git diff |
| Harbor Web | Web App | Codex Docs, Codex UI, Dev Server, Playwright |
| Beacon API | API | Editor, API Server, API Tests, Logs |
| Codex Platform | Services | Editor, Database, Migrations, Contract Tests |
| Harbor Ops | Codex Deploy | deploy.sh, Docker Compose, Service Logs, Health Checks |

Atlas Checkout retains the actual native linked-worktree relationship and branch
`feature/NO-TICKET-atlas-review`. Renaming its workspace did not create or rename
a Git branch or checkout. The earlier rename map initially called Harbor Web
“Codex Web”; the subsequent correction is recorded in
`/tmp/opencode/xrd/finishing/project-search-layout.json`. The final capture
snapshots and diagrams use **Harbor Web**, avoiding an unintended workspace-wide
`odx` match in that group.

The ten native `odx` results retain topology order: Codex Docs and Codex UI match
only the pane field in Harbor Web; four rows match workspace Codex Platform;
four match tab Codex Deploy in Harbor Ops. Agent-field matching is implemented,
but no agents or status behavior are demonstrated by these shell examples.

## Completed native takes

The owner captured the actual Herdr **0.9.3** client and popup through the existing
plugin route. All sources are silent H.264, **1656×812 at 25 fps**. VHS settings
request bash, DejaVu Sans Mono 16, a 160×40 terminal, padding 12, and no cursor
blink. The owner supplied a minimal sanitized environment with isolated HOME,
XDG directories and `HERDR_CONFIG_PATH` under `/tmp/opencode/xrd/`; the two tape
environment declarations alone would not establish isolation.

Requested popup outer size is 142×30. All six same-stem evidence files record
actual inner content **131×28**, measured using `TIOCGWINSZ` on fd 0 of the
matching binary descended from the owned server. All use binary SHA-256
`58f12f014d9c78c3f5ebc92e4c8867c1bbb71dc456d83eef49a78303b66bd2ec`,
from main `3ec8a60` plus the existing working-tree changes, not a clean release.

| Tape in this directory | Source under `/tmp/opencode/xrd/` | Raw duration / frames | Native observations | Host focus |
| --- | --- | ---: | ---: | --- |
| [project-opening.tape](project-opening.tape) | `project-opening.mp4` | 9.12 s / 228 | 41 | Editor unchanged |
| [project-navigation.tape](project-navigation.tape) | `project-navigation.mp4` | 22.48 s / 562 | 82 | Editor unchanged |
| [project-search.tape](project-search.tape) | `project-search.mp4` | 43.32 s / 1,083 | 137 | Editor → Codex UI |
| [project-worktrees.tape](project-worktrees.tape) | `project-worktrees.mp4` | 17.72 s / 443 | 67 | Editor unchanged |
| [project-help-themes.tape](project-help-themes.tape) | `project-help-themes.mp4` | 32.36 s / 809 | 108 | Editor unchanged |
| [project-closing.tape](project-closing.tape) | `project-closing.mp4` | 4.92 s / 123 | 33 | Editor unchanged |

Each source has a same-stem `-evidence.json`, with before/after snapshots, focus
trace, topology counts, agent counts, binary/tape hashes, dimensions and theme
files. The compositor checked all observations: six/seven/28 and zero agents
throughout, with no observation errors. Only search changes host focus from
Editor (`w3:p1`, Atlas Shop / Frontend) to Codex UI (`w5:p2`, Harbor Web / Web App).
These IDs are observed session evidence, not reusable capture constants.

The Help/theme take starts without an isolated theme file and ends with
`{"theme":"dracula"}`. It shows Select help with independent `focus` search,
Themes help from the list, Dracula table preview, nested Themes help returning
to that preview, return to the list and save without pane focus. Closing retains
the saved Dracula palette. Earlier cancellation/preflight checks remain separate
historical evidence; cancellation is not newly demonstrated in this final take.

The owner stopped only its isolated `film` server afterward. The cleanup record
reports `status: not running`, Editor focus and preserved topology:
`/tmp/opencode/xrd/finishing/project-server-cleanup.json`. No compositor restarted
the server or executed VHS, Herdr controls or Git operations. **Do not rerun these
tapes as-is:** their outputs and evidence already exist. New capture requires
fresh authorized paths and owner preflight.

### Native source and evidence SHA-256

The evidence file for each row is `/tmp/opencode/xrd/project-<section>-evidence.json`.

| Section | Source MP4 SHA-256 | Evidence JSON SHA-256 |
| --- | --- | --- |
| opening | `6807ce833aa2857a7328b28aa10c03fa2e4cda882d161b37237e4813fea1bf61` | `836ffbdfeb5af325b720d41a2fcb980dfd8987e82c64244c8f664f6a9c39b245` |
| navigation | `37be79b8daf50ad33d7f71cf6b586bb39ec156a51d588e03e23efd21876e2bb3` | `7aa90c59d77b8a3ed155bc4468e62963c8a2dae8d04874a34f7d6431e2a00e86` |
| search | `9dc4220efc7d5ad8d28b1f1e33e97173d95a96164bd24474b94dfeb5ffa3fea8` | `fd7e8da23e1b165be3803f95d12d3c78774dbe878fd53ccf6249e7d2880c0e50` |
| worktrees | `f201cb49e47d1b7ba87f30f881734db9837e386c4a62ea37008bc2edb84f1236` | `58797c363505ff3fcc5a622bd12785170b96f58cf18d51b5b247051886135ec5` |
| help-themes | `2efac157d79c818d79a2b73e3f3d448324bf6da034a5be0971c035a52194ecb3` | `047308b7728fcda4190794fcfdae43fdece2c20406714ef0789e22ce6eb2c747` |
| closing | `9e36f2346eaace0c3e4b82feee669cc89b85dc34c8106500934db92583fe2add` | `c236a44f55e041b7580041e6c1799814a40dead169a1198975972d11b2ed9ea6` |

## Composition and timing

Times below are content-relative at 25 fps, before the small container video
start offset. Each section retains the previous narration and section duration.
Native frames are selected or repeated in chronological order, with no terminal
painting, fake keybinding, fabricated selection, caption overlay or source crop.
H.264 encoding is lossy; no encoded pixel-identity claim is made. Edited timing
is not input latency or a performance benchmark.

| Section | Content interval | Frames | Edit |
| --- | --- | ---: | --- |
| Introduction and visible opening | 0–27.48 s | 687 | 550 diagram frames, then 137 frames retimed from the complete 228-frame opening take |
| Navigation | 27.48–50.12 s | 566 | 562 chronological native frames plus 4-frame final hold |
| Search | 50.12–82.60 s | 812 | Native query typing retained; filtered hold and confirmation phases retimed to the contract below |
| Worktrees | 82.60–99.60 s | 425 | First 395 source frames, before native close at 15.80 s; last actual Editor-selection frame held for 30 frames |
| Help/themes | 99.60–133.08 s | 837 | All 809 native frames plus 28-frame final hold |
| Closing | 133.08–151.56 s | 462 | All 123 native close frames, then existing full-frame editorial docs card for 339 frames / 13.56 s |

### Introduction and actual opening

`/tmp/opencode/xrd/finishing/project-tour/intro-projects.mp4` supplies exactly
22 seconds / 550 silent frames. Its title (0–3 s), nested real project tree
(3–11.2 s), project-labeled junction (11.2–17.3 s), and precise nested destination
(17.3–21 s) change only at meaningful events. The `odx` illustration underlines
ordered letters in Codex UI; one route is selected without a continuously moving
dot, progress marker or cyclic animation. The last second fades to dark.
Diagram phase names and narration association are editorial, not ASR timestamps.

At 22 seconds, the first native frame is the actual Atlas Shop / Frontend / Editor
host, not an already-open overview. The opening take visibly types and executes:

```sh
herdr --session film plugin pane open --plugin herdr-crossroads --entrypoint crossroads --width 142 --height 30; clear
```

This is an observed isolated capture route, not an installation command or
permission to operate a session. `; clear` is native shell cleanup, not simulated
UI. The 228-frame source is chronologically sampled into 137 output frames;
the actual typed command remains visible before the popup/table. The first full
native table is source frame 134 (5.36 s), mapped to content **25.24 s**. Native
popup creation includes ordinary redraw frames immediately before the full table.
No old native footage or terminal overlay is included in the diagrams. The next
separate navigation take starts with the same Editor selection and Auto palette.

### Search confirmation contract

Decoded source badge/row changes, not nominal VHS sleeps, establish:

| Observed action/state | Raw source frame / time | Final section frame / time | Global content time |
| --- | ---: | ---: | ---: |
| Search mode starts | 74 / 2.96 s | 74 / 2.96 s | 53.08 s |
| First Enter: Select, same filter and open popup | 886 / 35.44 s | 658 / 26.32 s | 76.44 s |
| `j`: Codex UI selected | 960 / 38.40 s | 703 / 28.12 s | 78.24 s |
| Second Enter: first native host repaint | 1,009 / 40.36 s | 766 / 30.64 s | 80.76 s |

The final mapping keeps source frames 0–199 unchanged, compresses only the long
filtered Search hold through source frame 885, then chronologically retimes each
native confirmation phase. All **812 decoded final search frames** were classified
against decoded source badge/selection templates and asserted to follow this
ordering. There is no native focus/host repaint before section frame 766.
The first host repaint is a genuine transitional redraw; the complete destination
is visible on the next output frame, **30.68 s** section-relative. Focus evidence
also comes from native snapshots, not solely from rendered borders or badges.

The conservative final-sentence end is the retained measured speech boundary
30.080136 s plus the existing 0.5-second lead, **30.580136 s**. First native focus
repaint at 30.64 s is after that boundary. This protects the explanation from early
focus; it is not a claim of exact word synchronization or listening approval.
The actual destination remains for the last **1.84 seconds** of the section.

Before the separately recorded worktree scene, the owner restores Editor focus
off-screen and reopens Crossroads with an empty query. That cut does not imply
that closing, focus confirmation or a theme save resets anything automatically.
The worktree edit omits its final native `q` close; closing is demonstrated later.

### Unchanged audio and container rounding

The entire original AAC stream is copied from `docs/media/crossroads-tour.mp4`,
not regenerated or re-encoded. Existing approved generation records, original
speech speed, section associations and 0.5-second leads are retained. Verification
compared all **6,536 AAC packets**, including integer PTS, DTS, duration, size,
payload SHA-256, flags and side data: every packet matches the original.
Extracted ADTS bytes also match, SHA-256:
`793d10dbcb19a4d60052b6574263e5e7dca7e7cc0cc90e101f35cb85d1f126d5`.

The MP4 video starts at **294/12800 s = 0.02296875 s**, rather than the original
297/12800 s = 0.023203125 s. The edit-list rounding difference is **−0.234375 ms**;
audio packet timing is exactly unchanged. Thus the first focus repaint is near
container **1:20.783**, not an exact container 1:20.760 assertion.

## Validation and retained evidence

The compositor used installed Pillow 12.1.1 and FFmpeg/FFprobe 9.0.2, with no
dependency installation. Commands ran from
`/home/user/workspace/herdr-crossroads`:

- `/usr/bin/python3 /tmp/opencode/xrd/finishing/project-tour/project-validation-source.py`
  — probes, full native decodes, native evidence and source samples passed.
- `/usr/bin/python3 /tmp/opencode/xrd/finishing/project-tour/project-validation-transitions.py`
  — decoded badge/row transition assertions and adjacent full-frame inspection passed.
- `/usr/bin/python3 /tmp/opencode/xrd/finishing/project-tour/composition.py`
  — fresh-output `ffmpeg -n` encoding/mux, exact frame/stream checks, full
  `ffmpeg -xerror` audio/video decode, all search-frame contract checks, audio packet
  and ADTS comparisons, and before/after preservation hashes passed.

These commands create evidence/media and must not be rerun against existing
outputs. Native tapes were already parsed during preparation; this composition
did not execute or revalidate VHS. Cargo/PTY suites were not rerun for this bounded
media/docs task; earlier software results are historical and not media acceptance.

Evidence under `/tmp/opencode/xrd/finishing/project-tour/`:

- `project-validation-sources.json`: actual dimensions, frames, durations, source
  hashes, snapshot counts, focus and theme state.
- `project-validation-transitions.json`: per-source-frame badge/selection states
  and measured source transitions.
- `project-validation-composition.json`: exact commands, final probe/hash, packet
  equality, all search output transitions, sampled RGB errors and preservation
  hashes for **101 existing artifacts**.
- `project-render-frame-map.json`: source path and source-frame index for every
  supplied output frame; native content is unpainted before encoding.
- `project-render-contactsheet.png` and 35 `project-render-final-*.png` samples:
  project diagrams, visible opening, pre/post confirmation, worktrees, contextual
  help, Themes preview/save and the closing card. Sample mean RGB error after
  lossy encoding stays below 1.19 on a 0–255 scale, not pixel identity.
- `checks/evidence/projects-render.json` and `projects-contactsheet.png`: corrected
  intro render, exact labels, semantic/stability checks and original-intro preservation.

The historical checks preserved earlier movies, raw takes, SRTs, GIFs, tapes and
both intro renders. Obsolete movie exports, subtitle drafts and two superseded
GIFs were later removed at the user's request; raw takes, evidence, tapes and
both intro renders remain retained. `crossroads-tour-clean.mp4` was the earlier
clean variant; `crossroads-tour.mp4` was the captioned audio source for this edit,
not a dependency of the current rebuilt circular-tour audio. The three unchanged
native GIFs use original names and separate provenance in [native-loops.md](native-loops.md)
and the [historical recording record](README.md#complete-six-section-candidate--2026-10-06).
Their editorial bands are not present in the current movie. Historical SRT timings
are not newly approved or applicable to this subtitle-free export.

## Open

- Independent visual/privacy review and revised-media acceptance are pending;
  the compositor's self-check is not independent review or release approval.
- Human listening must assess introduction association and the retimed navigation,
  search and Help/theme scenes. Measured boundaries do not establish subjective
  synchronization or exact word timing.
- Recheck current ElevenLabs Free-use terms, attribution and permitted use before
  any publication, then obtain explicit publication authorization. No publication
  or Git commit/push/PR was performed.
- Agent status, reconnect behavior, installation lifecycle, other platforms and
  input latency are not demonstrated by these ordinary-shell media examples.
