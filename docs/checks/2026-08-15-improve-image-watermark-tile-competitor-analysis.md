# image-watermark-tile — competitor analysis (2026-08-15)

Scope: the tool applies a repeating/tiled watermark across a whole image
(anti-theft), built as an ffmpeg block with a standalone page + CLI.
Research was a web scan of the tiled/diagonal-watermark category; no competitor
copy, wording, branding or trademark is reproduced here or in the tool. Findings
are recorded as capability facts only.

## Duplicate check (done first)

Not a duplicate. Nearest existing blocks were checked in source:

- `blocks/add-text-to-image` — draws **one** text run at an absolute `x`/`y`
  with no repetition and no rotation. Single caption, not a pattern.
- `blocks/image-composite` — overlays **one** image onto another once, with
  position/scale/opacity/blend mode. Single logo placement, no tiling.
- `blocks/pdf-watermark` — a different input type (PDF pages), not raster images.
- `blocks/image-split-overlay`, `blocks/image-opacity` — unrelated compositing.

No existing block repeats a mark across the full frame, so this ships rather
than being skiplisted.

## Category scan (top of the category, feature facts only)

Sources reviewed (URLs for provenance, no copy taken):

- https://makewatermark.com/
- https://www.allimagetools.com/add-watermark
- https://pixelpanda.ai/free-tools/add-watermark
- https://elysiatools.com/en/tools/image-watermark-tile
- https://www.onedocfreepdf.com/watermark-image

Observed table stakes across them:

| Capability | Seen in category | This tool |
| --- | --- | --- |
| Tile/repeat a mark over the whole frame | all | ✅ `columns` × `rows` |
| Diagonal placement | all | ✅ `angle`, default 30° |
| Adjustable rotation angle (not just a fixed 45°) | most | ✅ −90°…+90° |
| Opacity / transparency slider | all | ✅ `opacity` 0.02–1.0 |
| Text color picker | all | ✅ `color`, hex or CSS name, `kind = "color"` |
| Text size control | all | ✅ `font_size` 6–400 px |
| Spacing / span / density control | most | ✅ `columns`/`rows` (relative, 1–12) |
| Staggered ("checker-wise") vs aligned rows | some | ✅ `pattern = brick \| grid` |
| Local/in-browser processing, no upload | most claim it | ✅ ffmpeg-wasm on the page |
| Output format choice | some | ✅ `format = keep \| png \| jpg \| webp` |
| Logo/image watermark (not just text) | most | ❌ out of model — see below |
| Font family choice | most | ❌ out of model — see below |
| Batch / multi-file | some | ❌ out of model — see below |
| Live preview before download | most | ✅ page re-runs on every field change |

## Gaps closed in this build (in-model)

1. **Density is resolution-independent.** Tile centres are emitted as relative
   expressions (`x=w*0.125-text_w/2`), not pixels, so `4 × 5` looks the same on a
   400 px avatar and a 6000 px master. Category tools that expose a pixel "span"
   need re-tuning per image; this one does not. Only `font_size` is absolute, and
   the page copy states the ~2%-of-width rule of thumb.
2. **Rotation without bare corners.** The watermark layer is padded to 1.5×
   (≥ √2) before `rotate`, so a rotated pattern still covers the frame's corners
   — the exact region a thief would crop. The padded grid is generated at the
   same visible cell size, so density does not change when the angle does.
3. **Correct opacity compositing.** Glyphs are drawn fully opaque onto the
   transparent layer and the layer's alpha is scaled once at the end
   (`colorchannelmixer=aa=…`). Drawing at `fontcolor=white@0.3` instead
   double-blends against transparent black; measured output was ≈25 % white
   muddied toward black instead of the requested 30 %. Verified against real
   pixels: 50 % white over pure blue lands on exactly RGB (128, 128, 255).
4. **Brick layout leaves no crop gutter.** Alternate rows offset by half a cell
   and draw one extra tile so both edges stay covered — no clean vertical lane to
   crop along, which an aligned grid does leave.
5. **Legibility over mixed backgrounds.** `outline` adds a black border scaled to
   the font size, so one setting works over both bright sky and dark shadow.
6. **Presets.** Four `[[example]]` chips cover the recurring category presets:
   stock-photo `SAMPLE`, dense copyright proof, `CONFIDENTIAL` document stamp,
   and straight (0°) rows.
7. **Injection-safe text.** Text reaches ffmpeg via `textfile=` and the font via
   `fontfile=`, so quotes, colons, commas and `%` in a watermark are literal and
   the filtergraph stays a single space-free argv token.
8. **Format control with honest trade-offs.** `keep` preserves the container (and
   GIF animation); explicit conversions take the first frame and JPG pins
   `-q:v 2`. The page states that JPG output drops alpha.

## Out of model (listed, not built)

- **Logo / image watermark.** The generated tool page has exactly one file input
  and ffmpeg cannot run in the chat Service Worker, so a second uploaded image
  has nowhere to come from on the supported surfaces. Documented on the page with
  a pointer to `blocks/image-composite` for one-off logo placement.
- **Font family / weight choice.** Would mean bundling several TTFs into the wasm
  artifact for every page load; one bundled Liberation Sans Bold keeps the block
  small and renders consistently everywhere.
- **Batch / folder watermarking.** The page is a single-file upload; batching
  belongs to the CLI, which already loops from a shell.
- **Drag-to-position / freehand placement.** A tiled mark covers the whole frame
  by definition; per-tile dragging is not a meaningful control here.
- **Invisible / steganographic watermarking.** A different problem (robust
  imperceptible marks) needing a DCT/DWT engine, not a drawtext pattern.

## Known limitations recorded on the page

- Text ≤ 120 characters; density 1–12 across and down; size 6–400 px;
  opacity 0.02–1.0; angle −90°…+90°; input ≈25 MB.
- Very long text at a high tile count overlaps between tiles.
- Animated GIFs stay animated only with `format = keep`.
- A visible watermark deters casual reuse; it is not encryption. Keep a master.

## Surfaces

- **Page** (`/tools/image-watermark-tile/`) and **CLI** (`gizza tool
  image-watermark-tile …`) are the supported surfaces.
- **Chat is non-functional for ffmpeg blocks** (the runtime is a Service Worker,
  where `import()`/`Worker` are forbidden). The descriptor/schema is still
  validated by the drift-guard unit test, which is what chat would consume.
