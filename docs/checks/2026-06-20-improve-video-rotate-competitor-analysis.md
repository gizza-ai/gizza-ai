# video-rotate — competitor analysis (2026-06-20)

Twentieth `/create-next-tool` backlog pick. ffmpeg media tool — page + CLI.
Research via `WebSearch` (rotate-video tool landscape), paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| ezgif / VEED / Clideo | rotate 90/180/270, flip horizontal/vertical, in-browser | capabilities |
| Kapwing / online-converter | rotate + mirror, keep format, no watermark | capabilities |
| rotatevideo-style tools | fix sideways phone clips; preview before save | UX |

## Gap diff vs our tool
Our tool: rotate clockwise 0/90/180/270 (ffmpeg `transpose`) and/or flip
horizontal/vertical (`hflip`/`vflip`), combinable, re-encoded H.264 + copied
audio, keeping the container. Covers the full common feature set (rotate + flip +
combine). Rejects a no-op (rotate=0 & flip=none) and invalid values.

**At parity — nothing material to add.** Notes:
- Combining a rotation with a flip is supported (e.g. `transpose=1,vflip`).

**In-model gaps considered, deferred (minor):**
- **Lossless rotate via container metadata** — some tools just set the display
  rotation flag (no re-encode). ffmpeg can do `-metadata:s:v rotate=` / display
  matrix, but player support is inconsistent and it doesn't compose with flips;
  we re-encode for a predictable, universally-correct result. A `lossless` opt
  could be a future add for pure 90/270 with no flip.
- **Arbitrary-angle rotation** (e.g. 7°) with padding — needs the `rotate` filter
  + fill color; a separate, more advanced mode.

**Out-of-model:** preview-before-save UI (the page already re-runs on input).

## Tested
unit (8: rotate 90/180/270 transpose chains, flip h/v, rotate+flip combine, no-op
returns None + plan errors, invalid values error, plan keeps extension) +
drift-guard · `wafer build` validates the block · wasm-pack web · generator ·
Playwright page (uploads tiny-128x128.mp4 → in-browser ffmpeg → data:video) · CLI
on a real public video (rotate=90 → ffprobe confirms 640×360 → 360×640) + no-op
error path. Chat ffmpeg: non-functional — page + CLI.

> Original work only — no competitor copy, branding, or trademarks copied.
