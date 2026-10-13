# Historical circular-navigation tour provenance — 2026-10-09

This record describes the previous 164.92-second export, not the movie now at the
primary repository path. The current [calm demo record](calm-tour.md) supersedes
its timing, narration and destination. The previous export is retained only in
the local archive `/tmp/opencode/xrd/finishing/calm-tour/previous-circular-tour.mp4`,
with `previous-export-preservation.json` proving compressed audio/video packets
and timing preserved. It is not a published HTTP asset. All hashes, gates and
"current" descriptions below refer to this historical revision before replacement.
Technical media validation is separate from independent review, human listening,
visual/privacy acceptance and publication authorization. Those gates remain open.
Obsolete project, clean/captioned movie exports, subtitle drafts and two superseded
GIFs were subsequently removed at the user's request. The previous
[project-tour source record](project-tour.md), retained GIFs, raw takes, native
evidence and both introduction renders remain unchanged apart from documentation
reconciliation. Preservation hashes below record checks before that authorized
repository cleanup, not a claim that removed exports still exist.
Older GIFs predate circular navigation and do not demonstrate current wrapping.

## Composition

The content is 164.92 seconds (2:45), 4,123 frames at 25 fps, 1656×812,
H.264 video and mono 44.1 kHz AAC audio only. There is no subtitle stream,
burned-in text, bottom editorial band, mock UI or moving progress dot. Native
scenes use full, uncropped decoded frames in chronological order; pauses are
shortened or frames repeated to fit narration. This is not an input-latency test.

FFprobe container duration is `164.920000` seconds, file size 5,954,686 bytes.
Both video and audio start at `0.000000`; the first native search destination
therefore appears at container 1:34.120, not the previous edit's 1:20.8.
Movie SHA-256:
`605c37e126dd369f1a5ed9d407cb6e880c2ec9f217fd110f468bf0e685d27c0f`.

| Section | Content interval | Frames | Edit |
| --- | --- | --- | --- |
| Introduction/opening | 0–27.48 s | 687 | Preserved 22-second project diagrams, then all 9-second fresh visible opening retimed to 5.48 seconds |
| Navigation | 27.48–63.48 s | 900 | Fresh native selection and boundary examples retimed to 36 seconds |
| Search | 63.48–95.96 s | 812 | Fresh three-stage confirmation, exact relative frame contract below |
| Linked worktrees | 95.96–112.96 s | 425 | Trim before source frame 393 (15.72 s); hold actual Editor selection for 32 frames |
| Help/themes | 112.96–146.44 s | 837 | Fresh complete chronological sequence, holds reduced from 43.16 seconds |
| Closing/docs | 146.44–164.92 s | 462 | Fresh 4.88-second native close, then preserved editorial docs card for 13.60 seconds |

The diagrams associate real labels: Atlas Shop → Frontend/Tests, linked Atlas
Checkout → Checkout, Harbor Web → Web App, Beacon API → API, Codex Platform →
Services and Harbor Ops → Codex Deploy. Pane labels include Editor, Unit Tests,
Integration Tests, Type Check and Dev Server. Ordinary shell names and neutral
banners do not imply running developer workloads or agents. The visible native
opening starts with the actual host and typed plugin-launch command before Enter
and popup appearance; it is not an already-open replacement. The diagrams and
closing docs card are editorial illustrations, not native controls.

## Native captures and software evidence

All seven takes use binary SHA-256
`b09b37088b7e233d1c24eac6a8645c35dec114191bb127365704982e324f446b`.
The production writer reported five previous-boundary and five next-boundary
domain failures before its change and passing regression checks afterward. The
capture owner's `/tmp/opencode/xrd/finishing/circular-software-gates.log` records
220 unit, 11 integration and 27 startup tests passing, release compilation,
formatting and full PTY results. The caller reports all four complete PTY gates
and scoped diff checks passed. This media writer did not rerun those suites,
inspect production source/tests or independently approve that implementation.

Raw files are `/tmp/opencode/xrd/circular-<section>.mp4`, with same-stem
`-evidence.json` files and shipped [circular-preflight.tape](circular-preflight.tape)
plus six `circular-*.tape` scene scripts. All source movies are silent,
1656×812 at 25 fps. Six scene takes measure 131×28 inner content cells by native
PTY window-size observation; requested outer popup is 142×30 in a 160×40 terminal.
The separate preflight evidence does not contain its own inner-dimension measurement.

| Source | Seconds | Frames | Native sampled snapshots |
| --- | --- | --- | --- |
| Opening | 9.00 | 225 | 38 |
| Navigation | 38.20 | 955 | 124 |
| Search | 43.24 | 1,081 | 135 |
| Worktrees | 17.68 | 442 | 67 |
| Help/themes | 43.16 | 1,079 | 136 |
| Closing | 4.88 | 122 | 33 |
| Preflight, not included in movie | 36.96 | 924 | 121 |

Every sampled snapshot retains six workspaces, seven tabs, 28 panes and zero
agents. Only Search changes host focus from Editor (`w3:p1`) to Codex UI
(`w5:p2`, Harbor Web/Web App). All other scenes and preflight retain Editor.
These IDs belong to this isolated session; they are not reusable launch parameters.
Separate takes restore Editor off screen after Search. Navigation and preflight
leave the isolated theme file absent; Help/themes saves `{"theme":"dracula"}`,
and Closing retains it. Samples are not continuous instrumentation.
`/tmp/opencode/xrd/finishing/circular-server-cleanup.json` records the owned
server as not running and its saved topology preserved. This record authorizes
no restart, recapture or live control; outputs must not be overwritten.

### Circular-navigation observations

The owner inspected preflight decoded contact sheets and recorded observations in
`/tmp/opencode/xrd/finishing/circular-tour/native-preflight-inspection.json`:
full pane Editor → Health Checks → Editor; tab Frontend → Codex Deploy/deploy.sh
→ Frontend; workspace Atlas Shop → Harbor Ops/deploy.sh → Atlas Shop; Select
Help first next-pane entry → last close entry → first; Themes list and table
Auto → vesper → Auto; Themes Help first next-theme entry → last show-keys entry
→ first, returning to the pending table. Filtered `odx` panes wrap Codex Docs →
Health Checks → Codex Docs; filtered tab/workspace blocks wrap between Web App/
Harbor Web and Codex Deploy/Harbor Ops. Reset retains Codex Docs selection and
closing returns to the unchanged Editor host. No pane confirmation or theme save
occurs in preflight. Empty/single-entry behavior is software-test coverage, not
demonstrated by these movie examples.

The navigation edit keeps ordinary `j`/`k`, `l`/`h`, arrows and Tab/Shift+Tab
examples first. Its measured decoded boundary events are mapped as follows.
Times are relative to the navigation section, not exact spoken-word alignment.

| Native source frame | Edited time | Visible state/action |
| --- | --- | --- |
| 564 | 17.92 s | Pane first → last Health Checks |
| 594 | 23.44 s | Pane last → first Editor |
| 624 / 654 | 26.40 / 27.04 s | Tab last Codex Deploy / first Frontend |
| 684 / 713 | 27.68 / 28.32 s | Workspace last Harbor Ops / first Atlas Shop |
| 743 / 768 / 792 | 28.96 / 29.64 / 30.32 s | Open Themes Auto / last vesper / first Auto |
| 820 | 31.00 s | Cancel Themes, unchanged Editor |
| 824 / 849 / 873 | 31.64 / 32.44 / 33.24 s | Open Select Help / last close entry / first next-pane entry |
| 898 | 34.04 s | Return to Editor; hold through 36 s |

Help/theme examples finish after the last spoken sentence to allow readable
states rather than crushing all mode changes into that sentence. The separate
Help/themes section also shows list boundary moves, Themes Help boundary moves,
six forward theme steps to owner-verified Dracula, table preview, nested Themes
Help, its multi-entry `theme` filter with boundary moves, exact preview return,
list return and the genuine final save. Holds are shortened; native state order
is unchanged and no redundant action is represented as an invented save/cancel.

### Search contract

The fresh source phases were classified from every decoded frame using native
mode-badge templates and the second-row selected background. Ten `odx` matches
retain native order: two pane matches in Harbor Web/Web App, four workspace
matches in Codex Platform, four tab matches in Harbor Ops/Codex Deploy. There are
no agents, so agent-name matching is not demonstrated.

| Event | Fresh source frame | Section frame/time | Global content time |
| --- | --- | --- | --- |
| First Enter finishes editing, keeps filter/popup | 884 | 658 / 26.32 s | 89.80 s |
| `j` selects Codex UI | 958 | 703 / 28.12 s | 91.60 s |
| Second Enter, first native host repaint | 1,007 | 766 / 30.64 s | 94.12 s |
| Complete destination next frame | 1,008 | 767 / 30.68 s | 94.16 s |

First destination repaint is after the conservative measured final-sentence
end plus audio lead: 30.080136 + 0.5 = 30.580136 seconds. This is not ASR,
word-exact alignment or human listening approval. All 812 final Search frames
must match the expected source phase: no host before section frame 766. Content
times use frame zero as zero; actual container stream starts are retained in
the final FFprobe evidence, not silently copied from the previous movie.

## Approved audio, rebuilt without speed changes

Only Navigation changed in this historical edit: the user approved the exact
445-character text in the [historical storyboard](../demo-script.md#navigation-panes-tabs-workspaces--27486348-seconds)
and one Brian/`eleven_multilingual_v2`/`mp3_44100_128` generation.
`target/demo-narration/navigation-wrap-generation.json` records one HTTP 200
generation, no retry, and 30.371701-second mono 44.1 kHz MP3. Account lookup
returned HTTP 401; the user separately confirmed Free tier, sufficient free
quota and no paid overage. No account billing or remaining balance was observed.
Provider documentation was checked on 2026-10-09; current permitted use and
attribution still need checking before any authorized publication. No further
TTS, network or API operation occurred during composition.

The other five source MP3s retain their exact previous hashes. Six tracks are
decoded at normal speed, each with a 0.5-second silence lead, silence-padded to
its section and concatenated before fresh AAC encoding. The old whole-movie AAC
packet-identity result does **not** apply to this longer rebuilt audio stream.
Original narration, old `navigation.mp3`, metadata and both earlier renders remain
unchanged. The accepted wrap-up read-me pronunciation and pause are preserved
from `wrap-up-read-me.mp3`; no replacement closing sentence was generated.

| Source MP3 under `target/demo-narration/` | SHA-256 |
| --- | --- |
| `introduction.mp3` | `1f93b68d051ac8fe7f9bd44cff224ceceb7dfec5d2c27a55b3f8f4cb6a0382d4` |
| `navigation-wrap.mp3` | `ba0f085a6302b3b73ebdc980bde51a3ec771e4fea08d1223db973e66db77ae3b` |
| `search.mp3` | `11b4525331b7b4336c4e250271235c7b9f628b2adc44d1e1ae1f338ebbe05cec` |
| `context-worktrees.mp3` | `b667a0239381647e9259b7acc9da87d455fa297a4c8de98bc5255d48e7cd758d` |
| `help-themes.mp3` | `04215fccbd209e89a563caa2b21890dfa2b92573886c604e278848d7b8bd612c` |
| `wrap-up-read-me.mp3` | `2cae58436caf59dd4419e3c4a9e0c5a50949a09f74096ea0a7c996ebe742fd9f` |

## Source hashes

| Raw MP4 | SHA-256 |
| --- | --- |
| `circular-opening.mp4` | `8233017e7bc3137fe998bece5331d8ca63aab7aa11ee6a769459518f43167273` |
| `circular-navigation.mp4` | `2ceeb3be0b5057b19371d88779e93d08dd24345f18d66c35f006502a60a4548e` |
| `circular-search.mp4` | `ed4954fd8aa8274b5dae977c8457ce9ece15cc344ffdb9c81decb2284ee7979e` |
| `circular-worktrees.mp4` | `2d05397d1fc707b06171b2e21e9ee7499fa2b6c2e83726b708e98336a313fb97` |
| `circular-help-themes.mp4` | `aeec01b755f79917a065389ce320c91cd70cf8859ea0c3f24cfccc9520166bcd` |
| `circular-closing.mp4` | `d9b6137c3b1dbf651d04235633b4f7eb2bddb7482d108b2a23b1ad49e5975eaa` |
| `circular-preflight.mp4` | `8811bef87f563dd487ada244876b7daca6f5bfd3a8d778e1fea4c795081388d4` |

Full evidence-file hashes, tape hashes, original-media preservation hashes,
exact encoder/filter commands, probes, source/final phase arrays and every
output frame's source index are retained locally under
`/tmp/opencode/xrd/finishing/circular-tour/`.

## Validation and Open gates

From `/home/user/workspace/herdr-crossroads`:

```sh
/usr/bin/python3 /tmp/opencode/xrd/finishing/circular-tour/measure.py
/usr/bin/python3 /tmp/opencode/xrd/finishing/circular-tour/compose.py
/usr/bin/python3 /tmp/opencode/xrd/finishing/circular-tour/verify.py
```

These helpers refuse existing evidence/render outputs; do not rerun against
existing outputs. The composition records fatal-error full decoding, FFprobe
stream/dimension/rate/frame checks, all 812 final search-phase checks, sampled
full-frame RGB comparisons and before/after preserved-artifact hashes in
`circular-validation-composition.json`. `circular-render-frame-map.json` records
all 4,123 frames. `circular-render-contactsheet.png` contains decoded final
representatives for diagrams, visible launch, each navigation boundary, search
confirmation, Help/themes save and closing. The docs-only check separately
records local links/anchors and six exact approved narration comparisons.
No Cargo/PTY suite, live control, VHS capture or documented launch example was
executed by this media/docs writer.

**Open:** independent combined code/tests/docs/media review, human listening
(especially new navigation and search timing), visual/privacy acceptance,
current ElevenLabs permitted-use/attribution check and explicit publication
approval. Technical completion is not release approval. Native snapshots are
sampled; empty/single-entry cases and configurable-binding behavior are not
movie coverage. Removed SRT timing is not approved for this cut.
