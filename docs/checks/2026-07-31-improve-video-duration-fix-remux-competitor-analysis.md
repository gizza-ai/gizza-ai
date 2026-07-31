# Competitor analysis — video-duration-fix-remux (2026-07-31)

**Tool function:** rebuild a video container to repair missing/wrong duration
metadata (Infinity / 0:00 / N/A / mismatched header length) **without
re-encoding** — a stream-copy remux.

All notes are **paraphrased**; no competitor copy, branding, or trademarks are
reproduced. Out-of-model items are listed, not built.

## Scan

One WebSearch on "fix video duration metadata remux without re-encoding online
tool ffmpeg" plus a follow-up on the WebM-Infinity-duration failure mode. There
is **no crowded field of dedicated "fix video duration" web tools**; the space is
covered by (a) general remux/convert utilities, (b) one very popular JS library
purpose-built for the WebM-Infinity case, and (c) documented ffmpeg one-liners.
The three closest real references reviewed:

### 1. `fix-webm-duration` (open-source JS library, GitHub)
- **Function:** post-processes a `MediaRecorder` WebM Blob in the browser to inject
  the missing `Duration` element so the file reports a real length.
- **Params/UX:** takes the blob + a known duration in ms; no container choice, no
  transcode. Pure client-side.
- **Table-stakes surfaced:** browser-local / no upload; the exact failure mode
  (MediaRecorder WebM → `Infinity`); no quality loss.
- **Gap vs ours:** it *writes a caller-supplied duration* rather than measuring the
  true one, and only handles WebM. Ours measures the real duration by remuxing and
  handles any codec-compatible container.

### 2. General remux GUIs (MKVToolNix / "Remux"-style desktop apps, ff-utils)
- **Function:** repackage streams between containers without re-encoding
  (mkv/mp4/mov). Fixing a broken duration is a side effect of remuxing.
- **Params/UX:** container/target selection, stream selection, drag-and-drop, batch
  queue. Desktop installs.
- **Table-stakes surfaced:** container choice (mp4/mkv/mov), explicit "no
  re-encode / stream copy" promise, keep all streams.
- **Gap vs ours:** desktop install + broad muxing UI; ours is a single-purpose
  browser page focused on the duration repair.

### 3. ffmpeg documented one-liners (tecmint / VideoHelp / addpipe writeups)
- **Function:** `ffmpeg -i in -c copy out` to remux; `-movflags +faststart` for
  progressive MP4; `-fflags +genpts` for missing timestamps; `-cues_to_front 1`
  for WebM seeking.
- **Table-stakes surfaced:** stream copy remux, faststart (moov-to-front),
  genpts regeneration for broken PTS.
- **Gap vs ours:** CLI only; ours wraps the same flags in a page + chat/CLI tool.

## Table-stakes → decision

| Table-stake | In/out of model | Where it lands |
| --- | --- | --- |
| Browser-local, no upload, no quality loss (stream copy) | in-model | core `-c copy`; page copy states it |
| Fixes the MediaRecorder/screen-capture "Infinity duration" case | in-model | `regen_timestamps` (`-fflags +genpts`) + copy/example |
| Output container choice (keep/mp4/mkv/mov/webm) | in-model | `container` enum param |
| Keep all streams (video+audio+subs) | in-model | `-map 0` in core |
| MP4 web fast-start (moov atom to front) | in-model | `faststart` param (`-movflags +faststart`) |
| Regenerate broken presentation timestamps | in-model | `regen_timestamps` param |
| Worked example + stated limits/edge cases | in-model | `content.md` |
| Preset one-click chips (recorder-webm / streaming-mp4 / tolerant-mkv) | in-model | `[[example]]` chips |
| Set an ARBITRARY duration to a chosen value | **out-of-model** | Listed only — without re-encode this truncates/desyncs; JS lib does it by writing a caller-supplied value, which is not a *repair*. Documented in the FAQ as intentionally out of scope. |
| Batch queue / many files at once | **out-of-model** | Listed only — needs a server/desktop job runner; gizza is single-file browser-local. |
| WebM `-cues_to_front 1` seek index | **considered, rejected** | Niche vs faststart; folding a second "index-to-front" flag with different semantics per container would confuse the single `faststart` control. Container remux already rebuilds the WebM cues; not worth a separate param. |

## Result

Descriptor ships the in-model table-stakes from the start: `container`
(keep/mp4/mkv/mov/webm), `faststart` (default true), `regen_timestamps` (default
false), stream copy + `-map 0` in core, preset chips, worked example, and stated
limits. Out-of-model items (arbitrary-duration override, batch) are listed above
and, for the duration override, surfaced to users in the page FAQ.
