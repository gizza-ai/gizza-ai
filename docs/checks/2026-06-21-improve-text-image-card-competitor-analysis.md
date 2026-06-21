# text-image-card — competitor analysis (2026-06-21)

**Tool:** `text-image-card` — render a quote or short text onto a styled, shareable PNG card.
**Type:** pure-Rust (fontdue + `image`), image-bytes output. **Surfaces:** chat + CLI (no page —
image-bytes tools have no page render mode, same as `qr-code-generator` / `add-text-to-image`).
**Chat surface** validated by `wafer build` (instantiates clean, 908 KiB). **CLI** verified:
`gizza tool text-image-card text="Stay hungry, stay foolish." author="Steve Jobs" theme=sunset
width=800 height=600` → valid 800x600 PNG. **Unit + drift-guard tests:** 12 pass.

## Top competitors surveyed

1. **FastTool — Quote Image Generator** (fasttool.app) — 8 gradient background presets + solid colour +
   image-upload background; quote text + author name fields; Square/Story/Landscape size presets;
   browser-side (no upload); PNG download.
2. **QuickQuoteMaker** (quickquotemaker.com) — platform size presets (IG 1080×1080, Pinterest 1000×1500,
   TikTok 1080×1920); custom fonts/colours; free, no watermark.
3. **QuoteCrafter** (arcade.pirillo.com) — gradients, patterns, custom fonts; multi-platform sizes; runs
   fully in-browser, no account, no watermark.
4. **Digital Tool Pad — Quote Image Maker** — 10 gradient presets or a user photo as background.
5. **Text.imageonline.co / PikDraw / Canva / PosterMyWall** — heavyweight editors: template galleries,
   emoji/sticker libraries, drag-and-drop layout, AI background generation, stock-photo libraries.

## Gap diff (fit-to-model)

| Competitor capability | Status in `text-image-card` |
|---|---|
| Gradient/theme backgrounds | **Covered** — 6 themes (dark, light, sunset, ocean, forest, grape), each a vertical gradient. |
| Solid-colour background | **Covered** — `background_color` hex override disables the gradient. |
| Author / attribution byline | **Covered** — `author` renders a smaller accent-coloured "— Name" byline. |
| Custom text colour | **Covered** — `text_color` hex override. |
| Platform size presets (square/story/portrait) | **Covered via params** — `width`/`height` (200–4000) reach any preset (1080×1080, 1080×1920, 1000×1500, …). Defaults to 1080×1080 (the IG square). Named presets are a UX shortcut only; left out to keep the param surface lean. |
| Text alignment | **Covered** — `align` left/center/right. |
| Auto-fit long quotes | **Covered (edge over several competitors)** — greedy word-wrap + automatic font auto-shrink so long quotes stay inside the card; honours explicit `\n`. |
| Browser-side / local privacy | **Covered** — pure-Rust, runs locally on every backend incl. the chat Service Worker; text never leaves the device. |
| Multiple font families | **Out of model** — only one wasm-safe bundled font (DejaVuSansMono, shared with `add-text-to-image`); adding proportional/serif faces means bundling more TTFs (size cost). Not built. |
| Image / photo as background | **Out of model** — an image-bytes tool here has no second media file input; the chat SW + CLI image-bytes shape takes params only. Belongs to `add-text-to-image` (overlay onto a supplied image). Not built. |
| AI background generation, emoji/sticker libraries, template galleries, drag-drop editor | **Out of model** — these are full visual editors / GPU-model features, not a single pure-compute tool. Not built. |

## Verdict

`text-image-card` matches the core capability set of the dedicated quote-card generators (themes/gradients,
author byline, custom dimensions, alignment, colour overrides, local-only privacy) and adds automatic
word-wrap + font auto-shrink, which several free competitors lack. The unbuilt features (multi-font,
photo backgrounds, AI/editor features) are out of the pure-compute single-tool model and were intentionally
not added; the photo-overlay use case is already served by the sibling `add-text-to-image` tool. No
competitor copy, branding, or trademarks were used.
