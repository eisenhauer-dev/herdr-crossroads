# Historical calm Crossroads demo provenance — 2026-10-13

This record describes the superseded 120-second calm export, archived locally at
`/tmp/opencode/xrd/finishing/guided-tour/previous-calm-tour.mp4`. The same repository
movie path now contains the [guided edition](guided-tour.md), not this export.
All timing, hashes, choreography and checks below apply only to the historical
calm edition; its seven-second road-junction intro is not used in the guided cut.

The historical Crossroads demo follows one project:
Atlas Shop → linked Atlas Checkout → Checkout Editor. Technical validation passed;
human listening, independent visual/privacy review, final acceptance and publication
authorization remain separate, pending gates. The retained filename reflects the
accepted circular-navigation implementation and one small filtered wrap example,
not the previous broad keyboard tutorial.

## Export and composition

- **120.00 seconds (2:00), 3,000 frames, 25 fps, 1656×812**.
- H.264 video and mono 44.1 kHz AAC audio only; no subtitle stream, burned-in
  subtitles, bottom editorial band, crop or added black bars. Native key footers
  are real application UI and remain visible.
- Published-path SHA-256:
  `01930b8d83124fcf40b1eb43b7ab0db44b5d7a5cfd861d650af675859bf5a2c2`.
  Here "published-path" means the assigned repository file, not authorized remote
  publication. File size: 3,063,495 bytes.
- The fresh local candidate SHA-256 is
  `68325c9432eb066246821ed17ab82273f5e39bc6028dacc8599b5ebacbe0c93d`.
  Its remux into the assigned path has a different container hash; compressed
  packet payloads and timing are checked separately, not assumed from filenames.

| Section | Interval | Frames | Content |
| --- | --- | --- | --- |
| Introduction | 0–20 s | 500 | All 175 frames of the new seven-second editorial intro, then actual host, typed launch and overview |
| Navigation | 20–41 s | 525 | One Down to pnpm dev; one Tab to Checkout Editor, without host focus change |
| Linked worktrees | 41–58 s | 425 | Same continuous native view, linked marker and real existing branch |
| Search | 58–80 s | 550 | Gradual checkout query; first Enter; one filtered Up/Down wrap; second Enter; actual destination |
| Help/themes | 80–107 s | 675 | Second continuous take: Select Help, return, one Latte palette change, table preview, list return, save |
| Closing | 107–120 s | 325 | Genuine popup close, 2.96 seconds of Checkout Editor host, then retained editorial docs card |

The intro is editorial, not a terminal imitation: title/subtitle at 0–2 seconds,
a static selected Atlas Shop ↔ Atlas Checkout road junction at 2–4.5, then a small
linked workspace tree through 7 seconds. Atlas Shop contains Frontend/Editor and
Tests/Unit Tests; Atlas Checkout contains Checkout/Checkout Editor. Two gentle
one-time crossfades and a final dark fade are the only graphic motion. There is
no wandering point, progress marker or repetitive pulse. The native host begins
at 7 seconds; its real typed command precedes popup appearance at 12.28 seconds.

The first four sections use chronological decoded frames from **one native
program**, without reopening, off-screen focus resets or added UI painting between
Navigation, Worktrees and Search. Only native holds are compressed or extended by
frame selection/repetition; query typing and destination repaint retain their
original frame progression. This is not a real-time input-latency measurement.
Native scenes are full-frame. A direct editorial cut joins the destination host
to the second take's already-filtered list; no continuous app lifetime is claimed
across that cut. The docs card and introduction are explicitly editorial.

## Native sources and observed state

Both takes use the accepted binary SHA-256
`b09b37088b7e233d1c24eac6a8645c35dec114191bb127365704982e324f446b`,
Herdr 0.9.3, default keys, DejaVu Sans Mono 16, a 160×40 terminal with padding 12,
and requested 142×30 popup. Owner evidence measures **131×28 inner cells** by
native `TIOCGWINSZ`, not an estimate from the tape.

| Silent raw source under `/tmp/opencode/xrd/` | Seconds | Frames | Native observations |
| --- | --- | --- | --- |
| `calm-project-flow.mp4` | 68.68 | 1,717 | 202 |
| `calm-help-closing.mp4` | 29.36 | 734 | 102 |

Source SHA-256 values:

- Project flow: `dcb8cb174e32fca817cd684eb9ac6aae62aa37ab0a22c55a70f0784dbc9a9e0b`.
- Help/closing: `288ec2b9c5f4633f57fed8f41e0fe7dbc1f68267ed564efdb7c945332617ff86`.

Same-stem `-evidence.json` files contain native snapshots, focus traces, theme
observations, tape hashes and dimensions. Every sampled observation retains six
workspaces, seven tabs, 28 ordinary panes and zero agents. Only the final Select
Enter in Project flow changes host focus: Atlas Shop / Frontend / Editor (`w3:p1`)
→ Atlas Checkout / Checkout / Checkout Editor (`w4:p1`). These IDs are observed
session identifiers, not reusable launch arguments. Labels and neutral static
shell banners do not imply real editor, dev-server or test workload output.

The source workspace and linked checkout keep the real demo repository and
existing `feature/NO-TICKET-atlas-review` branch. No branch/workspace rename or
worktree creation occurs in this movie. Other project labels remain in the native
overview, but the edit does not navigate through those projects.

Help/closing starts with the actual Checkout Editor host retained. Reopening
Crossroads initially selects the first global row; hidden **real native**
`/checkout` and the first Enter reconstruct the four-row filtered selection.
This is setup for the second take, not proof that query/selection state persists
after closing and reopening the program. Help's `q` returns to that same in-app
query and selection. Auto → catppuccin → catppuccin-latte uses two quick native
steps but only one visible palette change. Space shows one table preview; Space
returns to the list; Themes Enter saves without a pane-focus request. The final
`q` closes Crossroads without changing host focus. The isolated saved file changes
from absent (Auto) to `{"theme":"catppuccin-latte"}`.

`/tmp/opencode/xrd/finishing/calm-capture-runtime.json` records the owner's minimal
isolated environment. `calm-server-cleanup.json` records the owned server stopped,
saved topology preserved and final focus on `w4:p1`. An initial pre-start
"not running" diagnostic is expected, not a capture failure. No personal session,
agent prompts or credentials are intended capture content; final privacy review
remains pending. These records do not authorize restarting or controlling Herdr.
Both [project-flow](calm-project-flow.tape) and
[help/closing](calm-help-closing.tape) outputs exist; do not rerun them as-is.

## Measured transitions and late-focus guard

Events come from decoded raw frames, not nominal VHS sleeps. Source indices are
zero-based; every exported frame's mapping is retained locally. Native frame
order is nondecreasing throughout each of the two programs.

| Event | Raw source frame | Movie frame | Movie time |
| --- | --- | --- | --- |
| Popup first repaint | Flow 132 | 307 | 12.28 s |
| Down selects pnpm dev | Flow 521 | 625 | 25.00 s |
| Tab selects linked Checkout Editor | Flow 667 | 725 | 29.00 s |
| Search editing begins | Flow 1,158 | 1,450 | 58.00 s |
| First Enter ends editing, retains filter/popup | Flow 1,447 | 1,700 | 68.00 s |
| Up wraps to last visible git diff | Flow 1,496 | 1,750 | 70.00 s |
| Down wraps back to first Checkout Editor | Flow 1,534 | 1,800 | 72.00 s |
| Second Enter, first native host repaint | Flow 1,571 | 1,886 | 75.44 s |
| Select Help opens | Helper 49 | 2,050 | 82.00 s |
| Help closes to filtered selection | Helper 170 | 2,175 | 87.00 s |
| Theme list opens | Helper 220 | 2,225 | 89.00 s |
| Latte palette appears | Helper 282 | 2,260 | 90.40 s |
| Table preview appears | Helper 378 | 2,325 | 93.00 s |
| Theme list returns | Helper 501 | 2,450 | 98.00 s |
| Themes Enter saves, returns to Select | Helper 537 | 2,488 | 99.52 s |
| q closes popup to unchanged Checkout Editor | Helper 660 | 2,675 | 107.00 s |

The four `checkout` results retain native order: Checkout Editor, pnpm dev,
Checkout Tests, git diff, all in Atlas Checkout / Checkout. One calm Up/Down
cycle wraps only these visible rows. There are no broad global boundary,
recursive Help or multi-palette demonstrations in this cut.

The conservative confirmation guard is **after the entire Search voice source**:
16.904127 seconds + 0.5-second lead = 17.404127 seconds. First host repaint is at
section frame 436 / 17.44 seconds, global **75.44 seconds**, and its native host
holds through 80 seconds. All 550 decoded final Search frames match Search,
Select, then Host phases; all 186 Select frames match first/last/first selection.
Detected audio silences are recorded, but no ASR, word-level sentence alignment
or human listening approval is claimed. The old 94.12-second confirmation and
old narration duration do not apply.

## Approved narration and cost provenance

The historical narration metadata contains the exact six approved texts in film order. Metadata under
`target/demo-narration/calm-*-generation.json` records **one request per text**,
Brian, `eleven_multilingual_v2`, `mp3_44100_128`, 1,470 total characters,
successful generation and pending human listening. The user confirmed Free tier,
sufficient free quota and paid overages disabled; no account balance or monetary
charge was independently observed. `calm-generation-batch.json` records the
approval and six successes. Composition made **no further generation, retry,
network/API, billing or dependency request**.

| MP3 under `target/demo-narration/` | Duration | SHA-256 |
| --- | --- | --- |
| `calm-introduction.mp3` | 14.860771 s | `8447aac5894ababe0915aeb5957c4fc6a26162ba00efb7867615ed65545926b0` |
| `calm-navigation.mp3` | 15.046531 s | `016516d1faa05356cca9b178bb0450c76698f2deabc3504dad9b538fa5b51300` |
| `calm-worktrees.mp3` | 14.814331 s | `169b3c8f76e44722a159041ae98ac58d9a32172bc4c783c86378c6f1864dc458` |
| `calm-search.mp3` | 16.904127 s | `7711547b14459a03c7d7df3a80bb74cee0d3b23e0b4c0e9312c8c704a14436be` |
| `calm-help-themes.mp3` | 12.492336 s | `4c4bbc55b88cec4b51be1aec7cab36670b050535c1a4195604d0a88515ee7af1` |
| `calm-closing.mp3` | 9.520181 s | `99dea5bd371eec1810eb76c82da4b6dd20b838bc57d1f2457bb66ff3bf84d9a4` |

Tracks decode at normal speed and pitch, receive a 0.5-second lead per section,
and are silence-padded to their sections before fresh AAC encoding. Identity with
the previous whole-movie AAC packets is neither expected nor claimed. Source MP3s
and generation metadata remain byte-identical. The retained closing card credits
voiceover by elevenlabs.io. Current Free permitted-use and attribution requirements
still need checking before separately authorized publication; the existing
noncommercial-use constraint has not been waived.

## Preservation, checks and Open

Only the existing primary repository MP4 was replaced; there is **one repository
MP4**. The previous export is archived outside the repository at
`/tmp/opencode/xrd/finishing/calm-tour/previous-circular-tour.mp4`, with
`previous-export-preservation.json`. Before replacement, the writer verified the
archive hash, all compressed audio/video packet payload hashes and PTS/DTS/duration
against the old primary, plus a full decode. The old export's SHA-256 was
`605c37e126dd369f1a5ed9d407cb6e880c2ec9f217fd110f468bf0e685d27c0f`;
container identity is not substituted for packet/timing proof. The archive is
local history, not a second published movie.

The three current native GIFs and three original published GIF/PNG assets retain
their exact hashes. All retained native tapes, old raw captures, audio sources,
generation metadata, the seven-second intro and existing docs card were checked
before/after composition. No obsolete MP4 or SRT was restored; historical records
remain [circular](circular-tour.md), [project](project-tour.md) and
[clean](clean-tour.md).

Commands run from `/home/user/workspace/herdr-crossroads`:

```sh
/usr/bin/python3 /tmp/opencode/xrd/finishing/calm-tour/measure-sources.py
/usr/bin/python3 /tmp/opencode/xrd/finishing/calm-tour/compose.py
/usr/bin/python3 /tmp/opencode/xrd/finishing/calm-tour/verify-candidate.py
```

These helpers require fresh output paths; do not rerun them over existing evidence.
Local `source-changes.json`, `frame-map.json`, `composition-evidence.json` and
`candidate-validation.json` contain decoded transition differences, all 3,000
source-frame mappings, exact render/audio/probe commands, source/protected hashes,
all Search phase checks and six silence-detection reports. Decoded final samples
and `render-contactsheet.png` were inspected by the writer. FFprobe verifies exact
streams, duration, dimensions, frame rate/count and zero video start time; full
FFmpeg `-xerror` decoding passed. Replacement-path packet/timing/hash and current
documentation checks are recorded in `publication-validation.json`.

No application code/tests changed and no Cargo/PTY suite was run by this media
writer. Main reports the unchanged accepted binary's 220 unit, 11 integration,
27 startup tests and all four complete PTY gates, plus formatting/build checks;
those are owner-supplied software evidence, not fresh media-writer release proof.
No Herdr/VHS control, Git mutation, network request or TTS occurred in this phase.

**Open:** independent human listening (especially timing), visual/privacy review,
final revised-media acceptance, current permitted-use/attribution check and explicit
publication approval. Snapshot evidence is sampled, not continuous instrumentation.
The film does not demonstrate configurable bindings, empty/single-row boundaries,
agent activity, installation or real development workload execution. Technical
validation and writer self-check are not independent acceptance or release approval.
