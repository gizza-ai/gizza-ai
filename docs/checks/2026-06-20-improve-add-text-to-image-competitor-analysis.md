# add-text-to-image — competitor analysis (2026-06-20)

Sixth `/create-next-tool` backlog pick. Pure-Rust image tool (fontdue + the
`image` crate + a bundled DejaVu font) — like blocks/code-screenshot it runs on
ALL backends including the chat Service Worker. Surfaces: **chat + CLI** (no
standalone page — the generated page has a text-out mode and an ffmpeg-media
mode, but no mode for a pure-Rust image-bytes output). Research via `WebSearch`,
paraphrased.

(Two picks were skiplisted before this one: add-audio-to-video needs two media
inputs, which the single-input page driver + descriptor model can't express.)

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| Watermarkly | hundreds of fonts; hex color; rotation 0-360 | capabilities |
| Kapwing | font, color, outline, drop shadow, multiple text boxes, drag-position | capabilities / UX |
| ImageOnline / AllImageTools | meme-style bold (Impact), size 10-200px, color picker, drag + rotate | capabilities / UX |
| Fotor / Canva | meme presets, watermark templates, rich editor | UX |

## Gap diff vs our tool
Our tool: overlay `text` (multi-line via \n) at `x`/`y` with `font_size` and a
`#rrggbb` color, returning a PNG. Covers the core "custom text + position + size
+ hex color" capability on chat + CLI.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **Text outline/stroke + drop shadow** — the meme-classic readability feature
  (e.g. Impact white-on-black). Cheap in pure Rust (draw each glyph offset in the
  stroke color, then the fill on top); the highest-value next add. Deferred only
  to keep this (already heavy) iteration bounded.
- **Background box behind text** — another readability option (semi-opaque rect).
- **Font choices** — we bundle one mono font; more fonts = more bundled TTFs
  (wasm size), so a curated 2-3 font set is a sized follow-up.
- **Rotation** — pure-Rust glyph rotation is more involved; deferred.

**Out-of-model:** drag-to-position canvas, multiple text boxes, live preview —
a rich custom editor UI, not a form/chat tool; hundreds of fonts (size).

## Tested
unit (6: draws-text-keeps-dims, color #rrggbb/#rgb/empty/invalid, multiline,
empty-text error, bad-image error, bad-color error) + drift-guard · `wafer build`
validates the block (pure-Rust → also works in the chat SW) · CLI on a real
public PNG (httpbin → valid 100x100 PNG out) + MIME-guard + empty-text error
paths. No page surface (pure-Rust image bytes); chat works (pure-Rust).

> Original work only — no competitor copy, branding, or trademarks copied.
