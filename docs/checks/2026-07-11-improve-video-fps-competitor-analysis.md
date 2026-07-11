# video-fps — competitor analysis (2026-07-11)

Tool: **Change a Video's Frame Rate** (`/tools/video-fps/`). Re-times a video to
a fixed target fps with frame drop/duplication via ffmpeg's `fps` filter,
re-encoding to H.264/crf 20 (audio copied, or AAC on container switch). Runs
100% in-browser (ffmpeg.wasm) / CLI; chat path is non-functional (ffmpeg can't
run in a Service Worker).

## Top competitors surveyed

1. **Veed.io — Change Video Frame Rate** — upload → pick a preset fps
   (24/25/30/60) → server-side re-encode → download. Login/watermark on the free
   tier; files uploaded to their servers.
2. **Clideo — Change Video Framerate** — dropdown of common rates, cloud
   processing, watermark until paid.
3. **FreeConvert — Video Frame Rate Converter** — fps dropdown + advanced codec/
   CRF options; cloud upload, size-capped free tier.
4. **Kapwing — Change frame rate** — editor-embedded; presets + custom fps;
   account required, cloud processing.
5. **ezgif — Video FPS changer** (part of the video toolset) — numeric fps box,
   server-side, small file-size cap.

## Feature diff (fit-to-model)

| Capability | Competitors | gizza video-fps | Verdict |
|---|---|---|---|
| Set arbitrary target fps | yes (box or preset) | yes (`fps`, 1–240) | **at parity** |
| Common-rate presets (24/25/30/60) | yes | yes — `[[example]]` chips (30, 24, 25) | at parity |
| Frame drop when lowering | yes | yes (`fps` filter) | at parity |
| Frame duplication when raising | yes | yes (`fps` filter) | at parity |
| Duration preserved (vs speed change) | yes | yes — documented, distinct from change-speed | at parity |
| Keep audio in sync | yes | yes — audio stream-copied (AAC on container switch) | at parity |
| Keep container / sane fallback | partial | yes — mp4/mov/m4v/mkv kept, else → MP4 | at parity |
| Privacy (local, no upload) | **no** (all cloud) | **yes** — 100% in-browser | **gizza advantage** |
| No login / no watermark | mostly no | yes | **gizza advantage** |
| CLI + LLM/chat descriptor surface | no | yes (CLI; chat descriptor) | **gizza advantage** |

## Out-of-model (intentionally not built)

- **Motion-interpolated fps up-conversion** (optical-flow / `minterpolate`, or
  ML frame interpolation like RIFE) — generates new in-between frames for
  genuinely smoother slow-mo. `minterpolate` is extremely slow and unstable
  under ffmpeg.wasm at the sizes we allow; documented as a limitation instead.
- **VFR → CFR analysis / telecine (pulldown) handling** — niche; the `fps`
  filter already produces constant frame rate output.
- **Batch / multiple files** — the page and single-input ffmpeg dispatch take
  one upload.

## Copy / UX gaps closed

- Preset chips for the common rates (30/24/25) mirror competitor dropdowns
  without copying their labels.
- Page copy explicitly distinguishes **frame-rate change (duration preserved)**
  from a **speed change**, and states drop-vs-duplicate + the no-interpolation
  limit — the top user confusion the competitor FAQs address.
- Privacy / no-upload / no-watermark stated as the primary differentiator.

No competitor copy, branding, or trademarks were reused.
