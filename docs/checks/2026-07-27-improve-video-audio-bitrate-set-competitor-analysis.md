# Competitor analysis — video-audio-bitrate-set (2026-07-27)

**Tool scope:** re-encode ONLY a video's audio track at a chosen constant bitrate
(64/96/128/160/192/256/320 kbps; AAC for mp4/mov/mkv, Opus for webm) while the
picture is stream-copied (`-c:v copy`, lossless). Single video input, single
bitrate enum, output keeps the input container, runs fully in-browser via wasm
ffmpeg with a 25 MB cap, nothing uploaded. Surfaces: standalone page + CLI (chat
ffmpeg is unavailable).

Very few web tools do this exact niche (audio-only re-encode, video stream-copied).
Most are either full **video compressors** that fold audio bitrate into a whole-video
re-encode, or pure **audio-bitrate changers** that output audio-only files. Both are
instructive; the audio-only tools are the closest UX/copy analog.

## Competitors surveyed

| # | Tool | Task fit | Bitrate options | Video handling | Notable UX |
|---|------|----------|-----------------|----------------|------------|
| 1 | **FreeConvert** (freeconvert.com/video-converter) | Closest mainstream match — Advanced Settings expose a standalone **Audio Bitrate** dropdown + codec/sample-rate/channels | ~32–320 kbps discrete | Leans to full re-encode (H.264/H.265); no clear video-copy passthrough | Server upload, drag-drop, "Advanced settings" disclosure, broad formats |
| 2 | **XConvert** (xconvert.com) | Rich bitrate control, strongest on audio-output tools | Presets Highest(320)/High(192–256)/Medium(128) + **custom 8–320**, CBR/VBR toggle | Whole-file compression re-encodes | No sign-up, batch, use-case hints beside presets |
| 3 | **VEED** (veed.io/tools/video-compressor) | Compression-first; bitrate at video level, not audio-only | Slider/field, no audio-only kbps list | Re-encodes whole video | **Real-time output-size estimate**, wide container support |
| 4 | **Clideo** (clideo.com/compress-video) | Preset-driven compressor | Named tiers (Basic/Strong/Superb); no audio kbps enum | Re-encodes whole video | Estimated size before processing; free tier **watermarks** + uploads (avoid) |
| 5 | **Media.io** (compress.media.io) | Settings panel adjusts audio quality + bitrate + format | High/Medium/Low + adjustable audio bitrate | Re-encodes; audio rides along | Upload-based, batch, quality presets, no watermark |
| — | **Notevibes Audio Bitrate Changer** (bonus, audio-only) | Best design template: 64/128/192/256/320 preset set, each with a use-case caption | 64–320 kbps presets | n/a (outputs MP3 audio) | **100% in-browser, no upload**, live size estimate, bitrate in output filename, no account/watermark |
| — | **123apps / Online Audio Converter** (bonus, audio-only) | Simple quality slider → 64/128/192/320 | 64–320 | n/a | Low-friction, no account, batch |

## In-model gaps → what we did

- **Discrete bitrate preset set (64/96/128/192/256/320)** — matches our enum exactly. ✅ already the descriptor enum; surfaced as a `<select>` with use-case labels.
- **One-line use-case caption per preset** (voice / speech / stereo / music / high / max) — ✅ added via `[input.labels]` in `page/meta.toml`.
- **Example preset chips** for one-click prefill — ✅ added 64 (voice), 96 (podcast), 128 (default), 192 (music).
- **"Video untouched / stream-copied" as the core differentiator** vs full-video compressors — ✅ emphasized in title, hero, and body copy; this is our genuine edge and the SEO angle ("shrink a video's audio without touching the picture").
- **Privacy / no-upload copy** ("runs in your browser, nothing uploaded, free") — ✅ in description, hero, and FAQ.
- **Container/codec notes** (keeps container; AAC for mp4/mov/mkv, Opus for webm) — ✅ in body + FAQ.
- **Bitrate/quality guidance** (which kbps for speech vs music) — ✅ "Picking a bitrate" section + worked example.

## Out-of-model (explicitly NOT built)

- Batch / multi-file processing (XConvert, Media.io, 123apps) — single-input page + wasm ffmpeg model.
- VBR/CBR toggle, V0 modes (XConvert) — we ship a single CBR-style enum.
- Whole-video compression (resolution, FPS, CRF, H.264/H.265) — we stream-copy the video by design; a separate video-compression tool covers that.
- Cloud/server upload, Drive/Dropbox/URL import, accounts, watermarks — all the compressors upload; we don't.
- Sample-rate/channel remapping, waveform editors, container transcoding, target-file-size mode, "AI smart" compression.
- Live estimated-output-size readout — the page generator is generic (no per-tool JS); the body copy explains the saving instead.
- Showing the input's current audio bitrate — would need probe-in-page JS the generic runtime doesn't provide.

## Verdict

Distinct, non-duplicate tool. Its edge over the mainstream compressors is that the
**picture is never re-encoded** — only the audio bitrate changes — which no surveyed
video compressor cleanly offers. All in-model competitor ideas (preset set, use-case
captions, example chips, privacy/no-upload framing, container/codec notes) are now
reflected in the descriptor and page copy. Nothing was copied verbatim from any
competitor.
