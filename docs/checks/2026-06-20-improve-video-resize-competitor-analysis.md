# video-resize — competitor analysis (2026-06-20)

Nineteenth `/create-next-tool` backlog pick. ffmpeg media tool — page + CLI.
Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| VEED / Media.io / CapCut | custom W×H, aspect-ratio lock, presets, in-browser, no watermark | capabilities |
| Absolutool / cleverutils | resolution presets 1080p/720p/480p/4K | capabilities |
| quso / ezgif | social presets (16:9/9:16/1:1), keep quality | capabilities / UX |

## Gap diff vs our tool
Our tool: scale to width and/or height; omit one and the aspect ratio is
preserved (the other side computed to an even number via ffmpeg `scale=…:-2`),
re-encoded H.264 + copied audio, keeping the container. Covers the custom-dimension
+ aspect-lock core competitors center on.

**In-model gaps considered, deferred (fit the model; UX conveniences):**
- **Resolution presets** (1080p/720p/480p/4K) — a `preset` param mapping to a
  target height; trivial future add over the existing scale logic.
- **Audio re-encode fallback** — we `-c:a copy`; a container that can't hold the
  source audio codec would need a re-encode. Rare; could auto-fallback.

**Out-of-model / other tools:** aspect-ratio CHANGE with crop/pad to 9:16 etc.
(that's cropping/padding — video-crop territory, not pure scaling); social-network
preset bundles (UX layer).

## Tested
unit (4: both dims, width-only `scale=W:-2`, height-only `scale=-2:H`, plan
validation + extension) + drift-guard · `wafer build` validates the block ·
wasm-pack web · generator · Playwright page (uploads tiny-128x128.mp4 → in-browser
ffmpeg → data:video) · CLI on a real public video (width=320 → ffprobe confirms
320×180, aspect preserved). Chat ffmpeg: non-functional — page + CLI.

> Original work only — no competitor copy, branding, or trademarks copied.
