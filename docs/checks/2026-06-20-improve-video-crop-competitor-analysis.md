# video-crop — competitor analysis (2026-06-20)

Fifth `/create-next-tool` backlog pick; first ffmpeg media tool of this batch.
Surfaces: standalone **page** (in-browser ffmpeg) + **CLI** (system ffmpeg).
Chat is non-functional for ffmpeg (the chat runtime is a Service Worker where
ffmpeg can't load) — stated, not claimed. Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| EZGIF | aspect-ratio presets (16:9, 4:3, 1:1, 9:16, 3:2) + freeform; precise X/Y/W/H | capabilities |
| Kapwing / Flixier / Clideo | draggable crop overlay, live preview, rule-of-thirds grid; presets | UX |
| ToolMagic / online-video-cutter | exact X/Y/W/H pixel inputs; browser-local | capabilities |

## Gap diff vs our tool
Our tool: `width`/`height` (required) + optional `x`/`y` offset (centered when
omitted), re-encoded H.264/AAC keeping the container. This matches the
**precise pixel X/Y/W/H** capability competitors highlight, on both the page and
the CLI.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **Aspect-ratio presets (16:9 / 1:1 / 9:16 / 4:3)** — doable without decoding
  the video using ffmpeg crop *expressions* (e.g. 1:1 →
  `crop='min(iw,ih)':'min(iw,ih)'`; 16:9 with landscape/portrait branches), but
  it needs careful expression building + validation, so it's a focused follow-up
  rather than a rushed add. The explicit W/H/X/Y already covers exact crops.
- **Draggable crop overlay + live preview / rule-of-thirds** — a rich custom
  page UI; the generated tool page is a simple form, so this is a page-template
  enhancement, tracked separately.

**Out-of-model:** timeline scrubbing, multi-clip editing, accounts.

## Tested
unit (4: centered argv, offset argv, partial-offset-defaults-to-zero, plan
validates + keeps extension) + drift-guard · `wafer build` validates the block ·
wasm-pack web · generator · Playwright page (uploads tiny-128x128.mp4, in-browser
ffmpeg produces a data:video/ output) · CLI on a real public video
(Big_Buck_Bunny → ffprobe confirms output is exactly 128x128) + MIME-guard error
path. Chat ffmpeg: non-functional (Service Worker) — page + CLI are the surfaces.

> Original work only — no competitor copy, branding, or trademarks copied.
