# video-autocrop-bars — competitor analysis (2026-07-19)

New-tool build; scan done BEFORE implementing. Sources skimmed (paraphrased only — no
competitor copy/branding reproduced):

1. **Deus.video "Remove Black Bars from Video"** (browser tool) — fully automatic, zero
   settings: upload → analyze → crop → download. Drag-and-drop upload, before/after
   comparison slider, download + reset buttons, no watermark, "keeps original quality",
   accepts mp4/mov/avi/webm/mkv, runs client-side in the browser.
2. **HandBrake** (desktop, the reference auto-crop implementation) — crop modes
   automatic / conservative / none / custom; auto-crop is ON by default; detection samples
   preview frames with pixel/frame thresholds (`--crop-threshold-pixels`, default 9;
   `--crop-threshold-frames`); crop dimensions are snapped for encoder compatibility.
3. **ffmpeg cropdetect workflow** (canonical CLI recipe, e.g. ffmpeg-cookbook.com) —
   two-pass: `cropdetect=limit:round:reset` detect pass reading `crop=W:H:X:Y` from the log
   tail, then a `crop=…` re-encode pass; `limit` (black threshold, default 24), `round`
   (snap dims, encoder-friendly), `reset` (re-detect for variable content); gotchas: dark
   scenes eat into content at high limits, H.264 yuv420p needs even dims, `-c copy` cannot
   crop (re-encode required).

## Table stakes → in-model / out-of-model

| Capability | Competitors | Tag | Where it landed |
|---|---|---|---|
| Fully automatic bar detection (letterbox + pillarbox) | all 3 | in-model | two-pass cropdetect union (`reset=0`) over the whole clip; last `crop=` line is the accumulated max box, so fades from black can only grow (never shrink) the kept picture |
| Zero-config default | Deus, HandBrake | in-model | both params have defaults (threshold 24, round 2) — upload alone does the right thing |
| Sensitivity control for dark/compressed bars | ffmpeg `limit`, HandBrake thresholds | in-model | `threshold` 0–255 (default 24), page slider |
| Encoder-friendly dimension snapping | ffmpeg `round`, HandBrake | in-model | `round` enum 2/4/8/16 (default 2 = exact bar removal, still H.264-safe even dims) |
| "No bars" honesty (don't silently re-encode a full-frame video) | HandBrake shows crop 0/0/0/0 | in-model | crop == input dims → clear "no black bars detected" message (block error / friendly page note), no pointless re-encode |
| Keep quality | Deus claim | in-model | libx264 CRF 18 (visually lossless tier) + audio stream-copy when the container is kept |
| Container handling | Deus format list | in-model | family `h264_out_ext` rule: mp4/mov/m4v/mkv kept (audio `-c copy`), anything else → mp4 + AAC |
| Before/after comparison slider | Deus | out-of-model | generic page renders the output player + download; a synced dual-video scrubber is a platform feature (same bucket as the deferred video scrubber work), not per-tool JS |
| Per-scene variable crop (`reset=N`) | ffmpeg | out-of-model | output frame size must be constant for a single crop pass; scene-by-scene reframing is an editor feature |
| Lossless crop (`-c copy`) | user wish in guides | out-of-model | impossible by construction: ffmpeg filters require re-encode (documented in page FAQ) |
| Non-black border colors | editingtools.io (images only) | out-of-model | cropdetect is luma-threshold black detection; colored-border removal is an image-tool concern (that competitor doesn't handle video either) |

## Design decisions

- **Two-pass in every surface**: chat/CLI block dispatches ffmpeg twice
  (`video-silence-cut` / `video-target-filesize-encoder` precedent); the page takes over via
  `page/custom.js` (`video-target-filesize-encoder` precedent) — detect pass via
  `ffmpegExec(..., "detect.null")` reading `resp.log`, decision in shared core
  (`crop_plan`), then the crop pass.
- **Union detection (`reset=0`) over the whole clip** rather than sampling 5 s from the
  middle: inputs are capped at 25 MiB so a full scan is cheap, and the union is the
  conservative choice (any real content in the bar area at any time keeps those pixels).
- **Boundary semantics measured, not guessed** (local ffmpeg 6.1): `threshold=255` →
  cropdetect emits negative w/h (`crop=-318:-238:…`) → mapped to a clear "whole frame reads
  as black" error; `threshold=0` on limited-range Y=16 black bars → no bars detected
  (strictest setting) → the friendly no-bars message. Round matrix on a 240×180-in-320×180
  pillarbox: round 2/4 → 240×180, round 8/16 → 240×176 (h snapped down, offset recentred);
  letterbox 320×180-in-320×240 at round 16 → 320×176.
- **Preset chips**: Default (24/2), Dark bars (48/2) for grey-ish compressed bars,
  Encoder-friendly (24/16) — mirrors the presets/modes competitors ship (HandBrake's
  automatic vs conservative becomes the threshold choice).

Verified surfaces this run: CLI (real URL fetch + exact no-bars/validation messages) and
page (Playwright: letterbox crop 320×240→320×180 decoded dims, pillarbox + every `round`
enum value, threshold boundaries 0/255, `.mov` secondary container kept, deep-link).
