# Guided Crossroads demo provenance — 2026-10-13

The current [Crossroads demo](../crossroads-circular-tour.mp4) follows Atlas Shop
→ linked Atlas Checkout → Checkout Editor. It replaces the
[historical 120-second calm edition](calm-tour.md) at the same repository path;
there is only one repository MP4. Independent final-film review passed and the
user accepted the current revision on 2026-10-13; the user has also explicitly
accepted the current movie for privacy. These supplied decisions cover the hash
below, including closing-credit removal. They are not a fresh review by this
documentation writer or permission to publish. See
[current acceptance and provider terms](../README.md).

## Export and chronological composition

- **101.20 seconds (1:41.20), 2,530 frames, 25 fps, 1656×812**.
- H.264 video and mono 44.1 kHz AAC audio, with the title
  `Crossroads Demo — voiceover by elevenlabs.io`.
- Repository file SHA-256:
  `5497401d6dc418ae6bb0d7069fd397f90a2dd60d9447728bc5d86940ad4ed054`;
  4,118,053 bytes. This identifies the local repository file, not remote publication.
- No subtitle stream, burned-in subtitles, editorial footer band, native crop,
  painted UI or added black bars. The application's actual key footers remain.
- The user accepted the preceding cut after independent review, and subsequently
  accepted this minor revision. It removes only the visible closing-card voiceover
  credit and its fade-in. The
  descriptor “A navigator for Herdr”, documentation pointers, diagrams and native
  UI are retained. Attribution remains in the README video link title and movie
  metadata. The [2026-10-15 terms check](../README.md#narration-use-and-attribution)
  records Free noncommercial-only use and required title attribution, not a blanket
  compliance guarantee.
- This finishing revision decodes the previous repository export: the original
  captures, graphics, scripts and external evidence are no longer available.
  Native frames retain that export's chronological order within each scene;
  stable holds are compressed or extended. Re-encoding introduces generation
  loss, not new native rendering. This is not an input-latency measurement.

The introduction is regenerated as an explicitly conceptual animation. The title
settles into a header; Editor, Unit Tests and Checkout Editor cards fade and move
into view during the terminals passage. The same cards move once into their
workspace/tab/pane hierarchy during the third sentence. One-time workspace, tab
and pane accents build the hierarchy, then settle; there is no looping movement.
Atlas Shop contains Frontend / Editor and Tests / Unit Tests; linked Atlas
Checkout contains Checkout / Checkout Editor. Diagrams are explicitly conceptual,
without fake terminal chrome. The introduction remains until a gentle blend to
the actual complete popup at **15.36–15.88 seconds**, centered at **15.62 seconds**
in the measured spoken pause (midpoint 15.626690 seconds). The fully visible popup
then holds **0.702857 seconds** before navigation speech. Startup is compressed:
this revision does not show the command or Enter and does not measure launch
latency. The former one-second minimum popup-ready hold is cancelled.

All editorial scene changes blend over **13 frame intervals / 0.52 seconds**,
the nearest whole-frame duration to the requested 0.5 seconds at 25 fps:

| Editorial boundary | Blend range | Frames (inclusive endpoints) |
| --- | --- | --- |
| Introduction → native popup | 15.36–15.88 s | 384–397 |
| Native selection → keymap example | 32.28–32.80 s | 807–820 |
| Example's default → custom mapping | 35.32–35.84 s | 883–896 |
| Keymap example → same native selection | 38.28–38.80 s | 957–970 |
| Destination host → second native program | 75.24–75.76 s | 1881–1894 |
| Native host → retained docs card | 92.76–93.28 s | 2319–2332 |

Native key-driven repaints are application updates, not editorial scene cuts.

Navigation demonstrates pane, tab, then workspace selection, without changing
host focus. The six-second full-frame “Example mapping” concept graphic appears
centered on the 32.56–38.56-second interval, during the rhetorical question and
customization passage at the end of navigation. Its default→custom blend illustrates ↓/J and ↑/K
becoming ↓/N and ↑/P. An eligible configuration would replace `select.next_row`
with `["n", "Down"]` and `select.previous_row` with `["p", "Up"]`.
Uppercase keycaps are physical labels; literal configuration characters remain
lowercase. No custom keymap was applied during capture. Configuration supports
one or two keys per action in navigation, help and themes; search editing remains
fixed. See [schema](../../keymaps.md#schema-and-replacement) and
[key syntax](../../keymaps.md#key-syntax), not the diagram, for configuration rules.

The first four voice sections share one native project-flow capture. After the
concept graphic, the edit resumes the same selected Checkout Editor view for
linked-worktree context, then search. There is no native reopening or focus reset
between navigation, worktrees and search. The second capture retains the actual
Checkout Editor host; hidden native `/cou` and the first Enter reconstruct its
filtered selection. This is not a claim that a query persists across app closure.

## Native events retained from the previous export

Frames are zero-based. The source column below refers to the byte-backed-up
previous repository export, not recovered raw captures. Original raw-frame
references and capture observations were historical owner evidence; this revision
can verify only the existing exported visuals and its own mapping.

| Event | Previous export frame | Movie frame | Movie time |
| --- | --- | ---: | ---: |
| Complete native popup, fully visible after blend | 370 | 397 | 15.88 s |
| Down selects pnpm dev | 464 | 464 | 18.56 s |
| Right selects Tests / Unit Tests | 514 | 514 | 20.56 s |
| Tab selects Checkout Editor | 565 | 565 | 22.60 s |
| Search editing begins | 1425 | 1425 | 57.00 s |
| `cou` complete, five filtered rows | 1441 | 1441 | 57.64 s |
| First Enter ends editing only | 1695 | 1695 | 67.80 s |
| Second Enter; first host repaint | 1841 | 1841 | 73.64 s |
| Second program, filtered list fully visible after blend | 1888 | 1894 | 75.76 s |
| Select Help opens | 1913 | 1913 | 76.52 s |
| Help closes to caller | 1992 | 1992 | 79.68 s |
| Themes opens at Auto | 2013 | 1997 | 79.88 s |
| Latte palette preview | 2038 | 2022 | 80.88 s |
| Themes pane-table preview | 2076 | 2060 | 82.40 s |
| First save directly from table, Select Latte | 2127 | 2111 | 84.44 s |
| Second real opening, saved Latte selected | 2145 | 2129 | 85.16 s |
| Auto preview restores dark Herdr palette | 2172 | 2156 | 86.24 s |
| Second save, Select Auto | 2200 | 2184 | 87.36 s |
| q closes to unchanged Checkout Editor host | 2270 | 2270 | 90.80 s |
| Retained editorial docs card, fully visible after blend | 2332 | 2332 | 93.28 s |

The literal query `cou` matches letters in order, not necessarily adjacently.
Five results remain in native topology order: Checkout Editor, pnpm dev,
Checkout Tests and git diff in Atlas Checkout / Checkout, then Harbor Web /
Web App / Codex UI. Native matching underlines remain visible. Checkout Editor
is selected before search and remains selected; the edit never moves into the
unrelated Codex UI row or performs a wrap cycle.

The first Enter only ends query editing. The final Select Enter changes actual
host focus after all Search speech: decoded AAC speech ends at 73.541973 seconds,
and first host repaint is at 73.64 seconds. The destination is unobscured for 1.60
seconds before the next blend. Help narration starts during that hold, keeping
its spoken section gap short.

Themes uses Auto → catppuccin → catppuccin-latte, with one visible light-palette
change. Space shows the pane table; Enter saves directly from that preview, with
no extra Space/list return. A real second Themes opening starts at saved Latte;
two previous-theme steps reach Auto and Enter saves Herdr's default. Neither
Themes Enter requests pane focus. The second palette transition meaningfully
restores the original dark look, not a no-op preview or save.

The Select hold before Themes is compressed by 16 frames / 0.64 seconds, moving
Themes ahead of “Preview a theme…” without reordering native preview/save/reopen
states. Themes is visible at 79.88 seconds. Source MP3 silence boundaries suggest
that phrase starts at 82.129274 seconds; even the preceding sentence boundary at
79.900408 seconds is later than the opening. These associations are conservative
inferences from the exact text and pauses, not listening or ASR. Auto's final
Select hold restores the 16 frames, leaving `q`, closing and all audio unchanged.

## Historical native capture and file-state evidence

The previous production record reports both sources using accepted binary SHA-256
`b09b37088b7e233d1c24eac6a8645c35dec114191bb127365704982e324f446b`,
Herdr 0.9.3 and default bindings. Capture geometry is DejaVu Sans Mono 16,
160×40 outer terminal, padding 12, requested 142×30 popup. Owner evidence measures
**131×28 inner cells** with native `TIOCGWINSZ`.

| Silent source under `/tmp/opencode/xrd/` | Seconds | Frames | Native observations |
| --- | ---: | ---: | ---: |
| `guided-project-flow.mp4` | 59.88 | 1,497 | 176 |
| `guided-help-closing.mp4` | 26.60 | 665 | 93 |

The now-unavailable same-stem `-evidence.json` files recorded focus, counts,
theme files, geometry and tape hashes. All 269 observations reportedly had six
workspaces, seven tabs, 28 panes and zero agents, with no observation errors.
Flow reportedly had exactly one sampled host-focus
transition, Editor (`w3:p1`) → Checkout Editor (`w4:p1`), at final confirmation.
All 93 Helper observations reportedly retained Checkout Editor focus. IDs were discovered by
the capture owner; they are not reusable constants or configuration examples.
Ordinary shell labels/static banners are not actual editor, server or test output.

The previous writer reported checking the full Helper file trace, not just endpoints:

| First sampled phase time in capture observation clock | Saved theme files |
| --- | --- |
| 0.008 s | Absent; Auto |
| 24.269 s | `{"theme":"catppuccin-latte"}` |
| 30.974 s | Absent again; Auto restored |

These observation-clock times include hidden startup and are not raw-video or
movie timestamps. No other theme-file value was reported. Historically, these
phases established both saves with decoded Themes/Select transitions; colors alone
are not persistence proof. `guided-server-cleanup.json` records the owner's film
server stopped, topology preserved, final Checkout Editor focus and no saved
theme override. `guided-capture-runtime.json` records the minimal isolated
environment. None of those external logs or raw files can be rechecked now;
exported colors are not fresh persistence or topology proof. This finishing
revision performed no Herdr control or VHS execution.

Both [flow](guided-project-flow.tape) and [helper](guided-help-closing.tape)
tapes remain, but their external outputs are missing; do not rerun them as-is.
Future capture requires separately
authorized fresh outputs and an owner preflight.

## Narration and actual spoken pauses

The [storyboard](../demo-script.md#current-guided-edition--2026-10-13) contains the
six exact active texts, **1,575 characters**. Four calm sources are unchanged;
two separately approved guided replacements total **598 characters** and each
has one successful Free request, with no retries. Brian / `eleven_multilingual_v2`
and `mp3_44100_128` are unchanged. The user confirmed sufficient Free quota and
paid overages disabled; no account balance or monetary charge was independently
observed. No TTS, API, network, billing or dependency operation occurred here.

| Source under `target/demo-narration/` | Duration | Movie placement |
| --- | ---: | ---: |
| `calm-introduction.mp3` | 14.860771 s | 0.20 s |
| `guided-navigation.mp3` | 23.173515 s | 16.56 s |
| `calm-worktrees.mp3` | 14.814331 s | 40.96 s |
| `calm-search.mp3` | 16.904127 s | 57.00 s |
| `guided-help-themes.mp3` | 14.396372 s | 75.16 s |
| `calm-closing.mp3` | 9.520181 s | 90.80 s |

The previous export mixed the full MP3 files at normal speed/pitch with those
placements and fresh AAC encoding. This finishing revision **copies that AAC
stream unchanged**: all compressed audio payload hashes, PTS/DTS/durations and
decoded PCM agree with the byte-backed-up source export. No audio was regenerated,
retimed, trimmed, normalized or re-encoded. All source MP3 and metadata hashes
remain unchanged; the exact six active texts remain in the storyboard.

All five section gaps are measured from the **final decoded AAC**, using FFmpeg
`silencedetect=noise=-35dB:d=0.12`:

| Boundary | Estimated speech end | Next estimated speech start | Gap |
| --- | ---: | ---: | ---: |
| Intro → Navigation | 14.670522 s | 16.582857 s | 1.912335 s |
| Navigation → Worktrees | 39.350272 s | 41.040862 s | 1.690590 s |
| Worktrees → Search | 55.412449 s | 57.059615 s | 1.647166 s |
| Search → Help/themes | 73.541973 s | 75.222993 s | 1.681020 s |
| Help/themes → Closing | 89.183492 s | 90.854218 s | 1.670726 s |

The fully visible popup hold to Navigation speech is **0.702857 seconds**. Final Closing
speech ends at 99.926168 seconds, with 1.273832 seconds of trailing silence.
These are amplitude-threshold estimates, not ASR, exact word boundaries or human
listening approval. Human acceptance is recorded separately above; the numerical
checks alone do not establish sentence/graphic alignment.
The latest 1–2-second pause requirement supersedes two-minute padding; no voice
speed change or long still was added to reach the old target.

## Closing-credit removal validation

The accepted source export is backed up byte-identically at
`/tmp/opencode/xrd/finishing/credit-free-tour/previous-accepted-tour.mp4`, SHA-256
`4a7b8f43e417334e6cb09a629e342123512ede66aaa388838eda0875d5d778ae`
(4,081,569 bytes). No files were deleted and no GIF was updated or recaptured.
The repository still contains only the existing primary MP4.

The pre-encoding edit is confined to the rectangle x=64–527, y=688–751 in frames
2320–2529. The decoded card's uniform background is RGB (14, 14, 22). During
frames 2320–2332 the replacement region blends the preserved previous export's
native host anchor at frame 2319 into that clean background, with the original
alpha `(frame - 2319) / 13`. It does not place an opaque rectangle over the host
fade. From frame 2332 onward the region is uniform card background. Frame 2319,
all earlier frames and pixels outside the rectangle are unchanged before encoding.
The video is re-encoded, so decoded video pixels elsewhere are not bit-identical.
No cuts, animation, timing, theme state, native footer or voice track were edited.

Commands run from `/home/user/workspace/herdr-crossroads`:

```sh
python3 /tmp/opencode/xrd/finishing/credit-free-tour/finish.py compose
python3 /tmp/opencode/xrd/finishing/credit-free-tour/finish.py verify
python3 /tmp/opencode/xrd/finishing/credit-free-tour/check-docs.py
python3 /tmp/opencode/xrd/finishing/credit-free-tour/finish.py install
```

These are execution records, not rerun instructions. `composition.json` records
the bounded pre-encoding change. `candidate-validation.json` and
`installed-validation.json` record geometry, streams, full error-strict decoding,
all 2,530 frame comparisons and audio identity. Outside the edit, the maximum
per-channel mean difference at 414×203 comparison scale is 1.758/255 or less.
All 210 decoded edited regions match the clean compositor region within a
2/255 per-channel mean tolerance against the compositor region converted through
the output's RGB/yuv420p/RGB color space; every full-card credit area is uniform.
The initial raw-RGB reference failed at frame 2323 (2.119/255 green-channel mean
error). Inspecting all fade regions identified color-conversion differences, not
an incorrect pre-encoding blend. Correcting the reference retained the original
tolerance; neither the candidate nor the assertion was weakened. Samples
at frames 2320, 2325 and 2350 were visually inspected. No OCR claim is made.
All 4,360 AAC packet payload hashes, PTS/DTS/durations and sizes match the accepted
source; decoded PCM SHA-256 is
`a58ebf3145ed7f2204b9834e98efe7b86a3f18cf539e8070931965b36458c04b`.
The five measured spoken gaps, intro fade/hold and 73.64-second focus event above
are preserved, not newly retimed. The installed movie is byte-identical to the
validated candidate and fully decoded again.

Before/after hashes protect 271 existing project and prior finishing files,
including narration, all retained GIF/PNG assets, tapes, prior backups, helpers
and evidence, excluding only the five allowed movie/docs targets. The plan was
not read. `documentation-validation.json` records local links, attributed README
title, one repository MP4 and preservation checks. No network, TTS, fresh capture,
Herdr control, dependency, Git operation, code/test/config edit or deletion occurred.

## Earlier animation revision preservation and validation

The previous production record described the external archive
`/tmp/opencode/xrd/finishing/guided-tour/previous-calm-tour.mp4` as verified against
the previous primary, including all compressed packet payload hashes and
PTS/DTS/durations. Its SHA-256 is
`01930b8d83124fcf40b1eb43b7ab0db44b5d7a5cfd861d650af675859bf5a2c2`.
That historical archive and its evidence are now missing, as are the original
raw takes and graphics; this revision does not claim to preserve or recover them.
Instead, a fresh **byte-identical** backup of the 101.20-second previous primary
is `/tmp/opencode/xrd/finishing/animated-tour/previous-guided-tour.mp4`, SHA-256
`7a34e8b4864033dec003f655f446ab2fb98b48574f0dce76dd17bcc41abd4e57`.
No second repository MP4 or obsolete SRT was created. Fresh before/after hashes
protect pre-existing tracked and nonignored project files except the five
permitted movie/docs targets (the plan is not read), plus all
`target/demo-narration` files: 157 protected files in total. This
includes the six retained repository GIF/PNG assets and all repository tapes.

Earlier commands run from `/home/user/workspace/herdr-crossroads`:

```sh
python3 /tmp/opencode/xrd/finishing/animated-tour/prepare.py
python3 /tmp/opencode/xrd/finishing/animated-tour/compose.py
python3 /tmp/opencode/xrd/finishing/animated-tour/verify.py
python3 /tmp/opencode/xrd/finishing/animated-tour/check-docs.py
python3 /tmp/opencode/xrd/finishing/animated-tour/verify.py --install
```

These are local execution records, not instructions to rerun over existing
outputs. Helpers use installed Python/Pillow and FFmpeg; no dependency was added.
Fresh `before-hashes.json`, `frame-map.json`, `animation.json`, `transitions.json`,
`candidate-validation.json`, `candidate-silence.txt`, `documentation-validation.json`
and `installed-validation.json`
under that temporary directory record source preservation, all 2,530 mappings,
movement/alpha progression, geometry, five decoded gaps and audio packet/PCM
identity. FFprobe checks exactly two streams, H.264/AAC, duration, dimensions,
25 fps, frame count and mono 44.1 kHz audio. Full `-xerror` decoding passes.
All 2,062 retained frames are compared against the **previous export**, at
thumbnail scale. All 84 editorial blend frames are checked against their
source-pair alpha progression. Sampled animation/blend frames are compared against uncompressed compositor
outputs at full size. These are generation-loss checks, not original-raw pixel
identity. Decoded samples are inspected in `final-contactsheet.png`. The installed
repository movie is byte-identical to the checked candidate and fully decoded.
No original evidence logs, original source-frame map or native file-state proof
were reconstructed. This is local replacement, not publication.

No application code/tests changed and no Cargo/PTY suite was rerun by this writer.
The previous production record reported owner-supplied accepted-binary evidence
covering 220 unit, 11 integration, 27 startup tests and all four PTY gates. Those
historical results are not fresh media-writer software or release proof.

**Open:** missing original external sources/evidence and explicit publication
approval. Current acceptance applies to SHA-256
`5497401d6dc418ae6bb0d7069fd397f90a2dd60d9447728bc5d86940ad4ed054`,
not just the preceding `4a7b8f43e417334e6cb09a629e342123512ede66aaa388838eda0875d5d778ae`
cut. The [provider-terms check](../README.md#narration-use-and-attribution) records
the user-confirmed Free/noncommercial scope and attribution, with account-level
and generation-time evidence limits. No installation, real workloads or agents
are demonstrated.
The keymap concept is not proof of a live custom configuration. Writer self-check
and technical media validation are not independent acceptance or release approval.
