# video-to-h264 — competitor analysis (2026-07-07)

Tool function: force-transcode **any** input video to the most universally
playable form — H.264 (High profile) in an MP4 container, `yuv420p` 8-bit
4:2:0 chroma, `+faststart`, AAC audio. The value is the "make this play
anywhere" normalize, not format conversion (video-transcode) or size reduction
(video-compress).

## Scan (3 real competitors, paraphrased — no copy/branding reproduced)

1. **Rotato browser converter** (`tools.rotato.app/convert`) — closest to
   gizza's model: fully browser-local (their MediaBunny engine), nothing
   uploaded. Controls: video codec (H.264/H.265/VP8/VP9), audio codec
   (AAC/Opus/MP3/Vorbis), container (MP4/WebM/MOV), audio resample rate
   (44.1/48/16 kHz). No CRF, resolution, or H.264-profile control. Simple.
2. **VideoProc Converter (desktop)** — feature-rich. Controls: resolution,
   bitrate, frame rate; two-pass / 1:1 auto-copy; device/target **preset
   profiles** (general, iPhone, Android, Sony, TV, YouTube, Facebook); subtitle
   & tag config; GPU-accelerated batch. Does not surface baseline/main/high as
   an explicit toggle in the reviewed page.
3. **AnyConv MP4→H264** (`anyconv.com`) — deliberately minimal: upload, convert,
   download; **100 MB** file cap; no exposed settings. Emphasises that MP4/H.264
   plays on essentially every player/browser/OS.

## Table-stakes → decision (every one lands in the descriptor or is listed)

| Capability | Feasibility | Decision |
|---|---|---|
| Force H.264 video codec (`libx264`) | in-model | **IN** — core, always applied |
| `yuv420p` 8-bit 4:2:0 (decode-anywhere chroma) | in-model | **IN** — forced, always applied (the headline normalize) |
| `+faststart` (moov at front, progressive web play) | in-model | **IN** — always applied |
| AAC audio (universal audio codec) | in-model | **IN** — always applied; no-audio input handled gracefully |
| Quality / CRF knob | in-model | **IN** — `quality` 1–100 → practical libx264 CRF 18–40 (default 75 ≈ CRF 24) |
| H.264 **profile** (baseline/main/high) | in-model | **IN** — `profile` enum (high default; baseline = max compat for old/embedded players, no B-frames/CABAC) |
| Target/device presets (iPhone/Android/YouTube…) as UX | in-model | **IN as `[[example]]` preset chips** — mapped onto our two knobs (Max-compatibility=baseline, Web/social=high default, High-quality=high@90) |
| Browser-local, no upload, no account | in-model | **IN** — gizza runs ffmpeg-wasm in the page; matches Rotato's privacy model |
| Resolution / downscale | in-model | **Considered, rejected** — overlaps the existing `video-resize` tool (family-invariant: one job per tool); linked from the page instead |
| Frame-rate change | in-model | **Considered, rejected** — not a compatibility concern for H.264/MP4; niche |
| Encoder preset (ultrafast…veryslow) | in-model | **Considered, rejected** — hardcoded `medium`; a speed/size tradeoff, not a compat knob; keeps the schema lean |
| Audio sample-rate resample | in-model | **Considered, rejected** — AAC at the source rate is already universal |
| Other output codecs (H.265/VP9) or other containers (WebM/MOV) | in-model | **Out of scope** — that's `video-transcode`'s job; this tool's whole point is H.264/MP4 |
| Batch, GPU accel, subtitle/tag editing, two-pass, cloud, logins | needs server/desktop | **Out-of-model** — listed, not built |

## Resulting descriptor

- `profile` — enum `high` (default) | `main` | `baseline`; friendly `<select>` labels.
- `quality` — integer 1–100 (default 75), slider control, → libx264 CRF 18–40.
- Media input via `url` ⊕ `ref` (chat/CLI) or file upload (page); output always `out.mp4`.
- Always: `-c:v libx264 -profile:v <p> -pix_fmt yuv420p -crf <crf> -preset medium -c:a aac -movflags +faststart`.

Limits stated on the page: 10 MiB in/out cap; always outputs H.264/MP4 (for
WebM use video-transcode; to shrink use video-compress; to change dimensions use
video-resize).
