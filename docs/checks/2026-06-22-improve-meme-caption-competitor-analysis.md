# meme-caption — competitor analysis (2026-06-22)

Tool: add a classic top and/or bottom impact-style caption to an uploaded image
and return a PNG. Bold letters with an outline, centered horizontally, auto-sized
to the image width (long lines wrap, ≤3 lines), placed near the top/bottom edges,
uppercased by default. Surfaces: chat skill block + CLI. No standalone page (a
pure-Rust image-bytes output has no page render mode — same shape as
blocks/add-text-to-image / blocks/code-screenshot). Pure-Rust (fontdue + the
`image` crate), so it runs on ALL backends including the chat Service Worker.

## Top competitors (caption step only)

Scope is strictly the "add a top/bottom caption to MY uploaded image" capability —
template galleries, GIF/video, social sharing, and accounts are out of scope.

1. **Imgflip Meme Generator** — pre-filled top/bottom fields + unlimited extra
   boxes; classic bold-impact default font, 1,300+ fonts plus device fonts;
   separate fill-colour and outline-colour pickers (auto/white/black/custom);
   black stroke on by default; draggable/resizable boxes; auto font-sizing; raster
   download.
2. **Kapwing Meme Maker** — Text tool, multiple layers, text over or above the
   image; 100+ fonts (custom upload); full colour picker; adjustable white/black
   outline (default white-on-black-outline); drop shadow / blur / opacity; drag +
   resize; raster download.
3. **Canva Meme Maker** — unlimited text boxes, freeform drag/resize/rotate; very
   large font catalog, any colour; outline/"hollow" + shadow via text effects;
   multiple export formats. General editor, more manual than a dedicated form.
4. **Make A Meme (makeameme.org)** — top/bottom canvas text + multiple boxes; ~50
   fonts; text-colour picker + separate outline-colour control; classic all-caps
   impact styling by default; resizable/alignable text; raster download.
5. **iLoveIMG Meme Generator** — direct text fields + multiple boxes; ~6 fixed
   fonts (Arial/Impact/Verdana/Courier/Comic/Times); font-colour + background;
   shadow + opacity instead of a true stroke; "text inside" vs "text outside"
   placement; adjustable size; raster download.

## Capability diff vs. this tool

| Capability | Imgflip | Kapwing | Canva | Make A Meme | iLoveIMG | This tool |
|---|---|---|---|---|---|---|
| Caption an uploaded image | yes | yes | yes | yes | yes | **yes** (url/ref) |
| Separate top + bottom text | yes | yes | yes | yes | yes | **yes** |
| Bold impact-style default | yes | yes | yes | yes | yes | **yes** (bold sans, OFL) |
| White fill + black outline default | yes | yes | partial (effect) | yes | shadow only | **yes** |
| Configurable fill colour | yes | yes | yes | yes | yes | **yes** (`text_color`) |
| Configurable outline colour | yes | yes | effect | yes | no | **yes** (`outline_color`) |
| All-caps default + toggle | partial | manual | manual | yes | manual | **yes** (`uppercase`, default true) |
| Auto font-sizing to fit width | yes | partial | no | partial | no | **yes** (shrink-to-fit) |
| Long-caption word wrap | yes | yes | yes | partial | partial | **yes** (≤3 lines) |
| Raster (PNG) output | yes | yes | yes | yes | yes | **yes** |
| No install / no account / private | no | account | account | no | no | **yes** (chat + CLI, in-sandbox) |
| Runs fully offline / client-side | no | no | no | no | no | **yes** (pure-Rust, no network for compute) |

## Gaps closed this round (in-model)

- **Uppercase toggle** — added `uppercase` (default true) so the classic all-caps
  look is the default but mixed case can be preserved (matches the manual-case
  behaviour competitors allow). Verified via core test + CLI render.
- **Colour controls** — added `text_color` and `outline_color` (`#rrggbb`,
  default white/black), matching the fill-colour + outline-colour pickers that
  Imgflip / Make A Meme / Kapwing expose. Verified via core test + CLI render.

The baseline (own-image caption, top+bottom, bold impact default, auto-fit, wrap,
white-on-black stroke, raster output) was already met by the initial build; this
round closed the two cheap, deterministic, in-model copy/UX gaps.

## Out-of-model features (intentionally not built)

These require an interactive canvas, a font-asset pipeline, or a page surface this
chat+CLI image-bytes tool does not have, so they are deferred (not built):

- **Free drag / resize / rotate positioning** and **arbitrary extra text boxes**
  beyond top/bottom — needs an interactive canvas editor; this tool is a
  fixed top/bottom layout driven by typed params.
- **Large / extensible font catalog + custom font upload** — would bloat the wasm
  and needs per-font asset management; this tool bundles one bold OFL face.
- **Per-box independent font/style settings** — same interactive-editor dependency.
- **Drop shadow / blur / opacity / background-fill effects** — additional render
  modes beyond the classic outline; not core to the impact-caption capability.
- **AI-assisted caption generation** — out of scope (the LLM already supplies the
  text in the chat surface).
- **Text-outside-image / whitespace-caption placement** and **multiple export
  formats** — layout/format variants beyond the single PNG impact-caption shape.

NOTE: no competitor copy, branding, or trademark was reproduced — only neutral
factual capability descriptions were used.
