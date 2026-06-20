# gif-to-mp4 — competitor analysis (2026-06-20)

Ninth `/create-next-tool` backlog pick. ffmpeg media tool: GIF (image input) →
MP4/WebM video. Surfaces: standalone **page** + **CLI**; chat ffmpeg
non-functional. Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| ezgif | GIF→MP4 and GIF→WebM, silent video out, no watermark | capabilities |
| convertico / Cloudinary | up to ~10x smaller; WebM 24-bit color vs GIF 256 | capabilities |
| restream / clipy | one-click, H.264, batch | UX |
| (codec notes) | VP9 (default) or VP8; bitrate/CRF tuning near-lossless..max-compression | capabilities |

## Gap diff vs our tool
Our tool: GIF → **mp4** (H.264, crf 26, +faststart) or **webm** (VP9, crf 32),
even-dimension Lanczos scaling (H.264/yuv420p require even dims), all frames
encoded (animation preserved). Covers the mp4+webm headline with quality-tuned
defaults; output is typically several times smaller than the GIF.

**In-model gaps considered, deferred (fit the model; minor):**
- **Quality/CRF knob** — expose a `quality` param mapping to `-crf` so users can
  trade size vs fidelity (we currently use sensible fixed defaults). Easy add.
- **VP8 option** for older-software compatibility (we use VP9).
- **Max-width downscale** to cap output size.

**Out-of-model:** batch multi-file upload (one file per page run / chat call),
server-side conversion.

## Tested
unit (4: mp4 H.264+faststart+even-scale, webm VP9, format parse/default,
plan rejects bad format) + drift-guard · `wafer build` validates the block ·
wasm-pack web · generator · Playwright page (uploads a generated tiny.gif fixture;
in-browser ffmpeg → data:video) · CLI on a real public GIF (giphy → ffprobe
confirms an h264 256x256 mp4) + bad-format error path. Chat ffmpeg: non-functional
— page + CLI are the surfaces.

> Original work only — no competitor copy, branding, or trademarks copied.
