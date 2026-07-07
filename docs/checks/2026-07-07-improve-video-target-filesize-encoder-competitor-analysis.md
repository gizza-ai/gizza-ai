# video-target-filesize-encoder — competitor analysis (2026-07-07)

Tool function: re-encode a video so the output lands **under a chosen file-size budget**
(target MB) by computing the H.264 video bitrate from the clip duration and audio budget,
then doing one encode. Distinct from `blocks/video-compress` (single-pass **CRF quality**
knob — no size target) and `blocks/image-resize-to-filesize` (images, not video).

## Competitors scanned (paraphrased — no copy/branding reproduced)

1. **FitToMB** (fittomb.com) — "enter duration + target MB, get the exact bitrate, compress
   in one click." Target presets: 5/10/15/20/25/50/75/100/500 MB. Audio default AAC 96 kbps,
   adjustable. Output MP4 (H.264+AAC), inputs MP4/MOV/MKV/WEBM. Adaptive downscaling
   (720p→480p) with a manual resolution cap and optional fps reduction (30→24). Claims
   **two-pass** H.264.
2. **VidShift** (vidshift.io/compress) — in-browser, no upload. Target MB with preset buttons
   WhatsApp 16 MB / Email 25 MB / Slack 100 MB. **Duration auto-detected** from the file.
   Resolution dropdown (Original default; downscale to 1080/720/480). Output MP4 only. Quality
   is derived purely from target-size + resolution (no separate quality slider). Worked
   guidance: ~23 MB email (2 min @720p), 15 MB WhatsApp (1 min @720p), 7 MB Discord free.
3. **CursorClip video bitrate calculator** (cursorclip.com) — target size in MB or GB; audio
   dropdown **No audio / 64 (voice) / 128 (standard) / 192 (high) / 320 kbps**; resolution +
   frame-rate + codec dropdowns; two calc modes (size↔bitrate). Platform framing (Slack,
   Twitter/X, Notion). Calculator-only (does not encode).

## Table-stakes → decision (every item lands in the descriptor OR is listed here)

| Capability | In/Out of model | Where it lands |
|---|---|---|
| Target size in **MB** (required) | in-model | `target_mb` number param |
| Platform/size **presets** (Discord 10, Email 25, WhatsApp 16, Slack 50…) | in-model | `[[example]]` preset chips |
| **Auto-detect duration** (no manual entry) | in-model | page reads `<video>.duration`; chat/CLI probe `ffmpeg -i` log |
| **Audio bitrate** choice incl. "no audio" | in-model | `audio_kbps` enum: none/64/96/128/192/320 (default 128) |
| **Resolution downscale / cap** (1080/720/480/360) | in-model | `scale` enum: keep/1080/720/480/360 → `scale=-2:H` |
| Output **MP4 (H.264/AAC)** for max compatibility | in-model | always mp4 out |
| Inputs MP4/MOV/MKV/WEBM/OGV | in-model | `Input::Video` + `accept="video/*"` |
| Show the **computed bitrate** in the result | in-model | result summary text |
| **True two-pass VBR** (better bit allocation) | OUT of model | gizza ffmpeg bridge is one `build_argv → ffmpegExec` per invocation with no persisted passlog across calls; we do **single-pass** `-b:v` + `-maxrate`/`-bufsize` (CBR-style) and say so. Not claimed as two-pass. |
| **Frame-rate reduction** (30→24) | OUT of model (by choice) | a secondary quality lever; bitrate + optional downscale already control size. Listed, not built. |
| Per-platform dedicated landing pages | out of scope | marketing pages, not a tool capability |

## Feasibility spike (2026-07-07, ffmpeg 6.1.1)

12 s 640×480 test clip w/ AAC audio. target_mb=1, audio 128 kbps:
`video_bps = (1·1024·1024·8·0.97 − 128000·12) / 12 ≈ 550 kbps`. Encode
`-c:v libx264 -b:v 550k -maxrate 550k -bufsize 1100k -preset medium -c:a aac -b:a 128k`
→ 508 KB output (under the 1 MB budget). Duration read from the `ffmpeg -i` log
`Duration: 00:00:12.00` (chat/CLI) and from `<video>.duration` (page). Confirms the
single-pass computed-bitrate approach lands under target. A 0.97 container/mux safety margin
keeps real content under the cap.

## Honesty notes carried into the page + descriptor
- Single-pass, not two-pass (stated on the page + in the summary; two-pass listed as a limit).
- Undershoot is expected on highly-compressible content (lands under, not exactly at, the cap).
- If the target is too small for the duration + audio (video bitrate would fall below the floor),
  the tool errors with a clear "raise the target or drop audio / lower the resolution" message.
