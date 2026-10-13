# Clean narrated tour

`crossroads-tour-clean.mp4` was a historical candidate, subsequently superseded by
the [circular-navigation tour](circular-tour.md) and removed at the user's request.
The edit and validation results below describe the completed historical render,
before authorized cleanup; sources and local evidence remain retained.
Technical checks were not independent review, human acceptance or publication approval.

## Edit

- H.264/AAC, **1656×812**, **25 fps**, **3,789 video frames**; video content
  151.56 seconds, container/audio 151.583220 seconds.
- No burned-in narration subtitles, embedded subtitle track or appended bottom
  editorial bar. The entire former 150-pixel band is removed, including its key
  labels. Native application/host footers remain part of the real terminal image.
- The first 687 frames (27.48 seconds) introduce Crossroads with dark editorial
  diagrams: title and animated roads junction, Workspace → Tab → Pane, highlighted
  Search → Select → Jump, then the junction metaphor and a combined hierarchy/path.
  Chapter changes follow the existing introduction's sentence-pause anchors plus
  its 0.5-second audio lead; this is sentence-level association, not word-level ASR.
  A smooth 23.8–25.0-second dissolve reveals actual native frames, followed by the
  native overview until navigation begins. Diagrams are not Herdr UI or native
  capture. No full narration text or simulated terminal controls are drawn.
- After the introduction, each original edited frame is cropped to its top
  1656×812 native/documentation-card region. No further retiming or action edits
  occur. The closing editorial docs card remains, without the former bottom band.
- The existing Brian English AAC stream is copied, not regenerated, filtered,
  re-encoded or speed-changed. All six sections and their audio leads remain.
  No new capture, network request, dependency or software change was needed.

| Section | Video-content start | Duration |
| --- | ---: | ---: |
| Introduction | 0 s | 27.48 s |
| Navigation | 27.48 s | 22.64 s |
| Search | 50.12 s | 32.48 s |
| Linked worktrees | 82.60 s | 17 s |
| Help/themes | 99.60 s | 33.48 s |
| Closing | 133.08 s | 18.48 s |

These are edited media timings, not input-latency measurements. Navigation retains
the earlier chronological retiming; worktrees retain the off-screen reset to
Welcome after the search destination and the trim before popup closure. Other
native holds and the closing docs card are unchanged. Per-source binary hashes,
capture isolation, actual 131×28 popup dimensions and focus evidence remain in the
[historical capture record](README.md#complete-six-section-candidate--2026-10-06).

## Preservation and validation

The original `crossroads-tour.mp4` and its 28-cue `crossroads-tour.srt` were
preserved during these historical checks, then removed in the authorized cleanup.
Narration files and native sources/evidence remain unchanged. The historical SRT
was not embedded or burned into the clean movie; its timing was not newly approved.

Focused commands ran from `/home/user/workspace/herdr-crossroads` using installed
Pillow 12.1.1 and FFmpeg 9.0.2. The local helper and full command/evidence records
are retained under `/tmp/opencode/xrd/finishing/clean-tour/`:

```sh
/usr/bin/python3 /tmp/opencode/xrd/finishing/clean-tour/render.py
/usr/bin/python3 /tmp/opencode/xrd/finishing/clean-tour/validate.py
```

These commands were executed once against fresh outputs; **do not rerun them
against the existing candidate**. No media source was overwritten.

- FFprobe confirmed dimensions, codec, duration, frame count and exactly two
  streams (video/audio), with no subtitle stream.
- `ffmpeg -nostdin -v error -xerror -i docs/media/crossroads-tour-clean.mp4 -f null -`
  fully decoded the finished movie successfully.
- All **6,536 AAC packet payload hashes, PTS, DTS, durations and side data** match
  the original. Extracted ADTS AAC files are byte-identical, SHA256
  `793d10dbcb19a4d60052b6574263e5e7dca7e7cc0cc90e101f35cb85d1f126d5`.
- Native RGB supplied to the encoder is exactly the original cropped RGB for all
  **3,102 frames after the intro**, checked by matching cumulative hashes.
  Re-encoded H.264 is lossy, not pixel-identical: aligned post-intro SSIM has
  mean **0.999238**, minimum **0.998627** across all 3,102 frames.
- Source movie/SRT, six raw native takes, their native-evidence files and six
  narration-file hashes match before/after. Intro helper assertions check easing
  boundaries and changing imagery within each animated chapter.
- A decoded contact sheet at 1, 5, 9, 13, 18, 22, 26, 28, 60, 104, 114 and
  140 seconds was inspected for diagrams, real terminal content and removal of
  the entire bottom band. Full-size hierarchy/path/junction and native frames
  were also inspected. This self-check is not independent review.

## Open

- MP4 edit-list rounding changes video start from 297/12800 to 294/12800 seconds
  (0.234375 ms earlier). Frame count, frame intervals, section lengths, audio
  start and every audio packet are unchanged. Exact original video-start PTS is
  therefore **not** claimed; this small alignment difference remains disclosed
  for acceptance.
- Independent revised-media review, subjective animation/legibility approval and
  human listening remain pending. No new listening or voice-quality approval is
  inferred from packet equality.
- Existing software gates were supplied as passed (207 unit, 11 integration,
  27 startup and all four PTY gates). They were not rerun for this media-only edit;
  this revision does not establish new software-release acceptance.
- Publication still needs explicit authorization and a fresh ElevenLabs Free-use
  terms/attribution check. No installation or capture examples were executed.
