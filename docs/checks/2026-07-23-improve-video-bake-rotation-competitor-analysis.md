# Competitor analysis — video-bake-rotation (2026-07-23)

Tool goal: take a video whose orientation is carried only by a rotation *flag*
(display-matrix / `rotate` tag) — the sideways-iPhone-clip problem — and bake that
rotation into the actual pixels, re-encoding upright and clearing the flag so every
player (including the ones that ignore the flag) shows it the right way up.

## Search

One `WebSearch`: "bake video rotation metadata into pixels fix sideways video
autorotate ffmpeg tool online". The space is dominated by ffmpeg how-to guides
(the exact recipe this tool automates), not one-click hosted tools. Top 3 real,
reachable sources analysed below (paraphrased — no copy reused).

## Top sources / competitors

### 1. Mux — "How to rotate videos using FFmpeg" (mux.com)
- **Approach:** documents both the metadata-only rotate (`-c copy -metadata:s:v:0
  rotate=0`) and the pixel re-encode via `transpose`. Frames the two as a tradeoff:
  metadata-only is instant/lossless but player-dependent; re-encode is universal.
- **Params surfaced:** transpose direction, audio `-c:a copy`.
- **Gap vs us:** it's a guide, not a tool; user must know their file already carries
  a flag and pick the transpose value by hand. Our tool auto-detects the embedded
  rotation (ffmpeg autorotate) so the user supplies nothing but the file.

### 2. mpegflow — "Rotate and flip video with FFmpeg: phone footage, EXIF metadata,
   and the right approach" (mpegflow.com)
- **Approach:** explicitly covers the phone-footage case where the rotation lives in
  the container's display matrix and players disagree; recommends re-encoding to
  "bake" orientation rather than trusting the flag. Notes autorotation is applied on
  decode by default.
- **Params surfaced:** notes CRF/preset for the re-encode quality; audio stream-copy
  when container-compatible.
- **Gap vs us:** manual command construction; no container/audio-compat handling
  (webm→mp4 fallback) — our core reuses `h264_out_ext` to keep the container when it
  can hold H.264+AAC and switch to mp4 (AAC re-encode) otherwise.

### 3. ffmpeg-cookbook — "FFmpeg Rotate Video — Fix iPhone Portrait Mode and 90°/180°
   Rotation" (ffmpeg-cookbook.com)
- **Approach:** step recipes for the iPhone portrait-mode sideways bug; shows the
  `transpose=1,transpose=1` chain for 180° and stresses stripping/zeroing the rotate
  tag so it isn't double-applied after baking.
- **Params surfaced:** explicit rotate angle (90/180/270), `-metadata:s:v:0 rotate=0`.
- **Gap vs us:** requires the user to state the angle; ours reads whatever angle the
  file already declares and applies exactly that, then zeroes the flag.

## Gap synthesis → what we build (all in-model)

- **Auto-detect the embedded rotation** (no angle param): ffmpeg autorotate reads the
  display matrix / `rotate` tag on decode and applies it while re-encoding. This is
  the whole point and the key differentiator vs the explicit-angle `video-rotate`.
- **Clear the flag** after baking: `-metadata:s:v:0 rotate=0`; autorotate also consumes
  the display-matrix side data, so the output carries no rotation (verified: a 128×64
  clip with a -90 display matrix bakes to a 64×128 upright file with empty side data).
- **Container/audio compatibility:** reuse `h264_out_ext` — keep mp4/mov/mkv/m4v and
  stream-copy audio; webm → mp4 with AAC re-encode.
- **Quality knobs (crf/preset):** competitors mention them but expose them as raw
  ffmpeg flags. Kept as fixed sensible defaults (libx264, preset medium, crf 23) to
  match the gizza video family (video-rotate does the same) and keep the tool
  one-click. Considered, deliberately not exposed as params (schema bloat, minimal-UX).

## Out-of-model (not built)
- Batch/folder processing, cloud rendering, accounts/API keys — need a backend.
- Metadata-only (lossless) rotate toggle — that's the flag-trusting path this tool
  deliberately avoids; the explicit-angle `video-rotate` tool covers user-driven
  rotation.

> Original analysis; no competitor copy, branding, or trademarks reproduced.
