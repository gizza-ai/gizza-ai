# pdf-watermark — competitor analysis (2026-07-10)

Tool: stamp a **text** watermark onto every (or selected) page of a PDF — the classic
CONFIDENTIAL / DRAFT / company-name overlay. Pure Rust (`lopdf`, base-14 fonts), chat + CLI
only (binary-in / PDF-out has no standalone page render mode, same as `pdf-page-numbers`).

Not a duplicate of `pdf-page-numbers`: that tool prints an incrementing sequence
(`{n}`/`{total}`) in a page corner, upright, opaque-black by default — a page numbering utility.
This tool stamps one fixed string identically on every page, large + faint + **rotated**
(diagonal by default), optionally **tiled** across the whole page — a document-marking utility.
Rotation and tiling are the defining watermark features and are absent from `pdf-page-numbers`.

## Competitors surveyed (paraphrased — no copy/branding reused)

1. **iLovePDF** — text or image; font, size, colour, "font shadow"; transparency presets
   (none / 75 / 50 / 25 %); rotation presets (none / 45 / 90 / 180 / 270°); a 3×3 position grid
   plus a **Mosaic** mode (repeat across page); page range.
2. **Sejda** — drag-to-place text watermark with a rotation handle and resize handles; colour,
   font, transparency; precise X/Y coordinates; page range.
3. **Adobe Acrobat** — text or image; font/size/colour; rotation (-45 / none / 45 / custom);
   transparency (default 100 % opaque); **mosaic** = a 3×3 matrix of the watermark; page range.
4. **FunPDF** — text: font (Helvetica / Times Roman / Courier), size, colour, opacity, rotation;
   nine preset positions **or** custom X/Y %; Single / Tile / Diagonal layout modes; page range.
5. **Pi7 / WebToolTrix / MyToolsFree / FWD Tools** — text: Helvetica/Times/Courier, colour,
   size, opacity, rotation; centre-diagonal (classic -45° DRAFT) vs **tiled** grid; corner/edge
   positions; page range. Consensus defaults: 15–40 % opacity, ±45° rotation, grey.

## Table-stakes → model-fit mapping

| Capability | In model? | How we ship it |
|---|---|---|
| Watermark text | ✅ | `text` (required) |
| Font family (Helvetica / Times / Courier) | ✅ | `font` enum — base-14, no embedding |
| Font size (points) | ✅ | `font_size` (4–288, default 48 — large, watermark-scale) |
| Colour (hex) | ✅ | `color` (default `#808080` grey, per competitor consensus) |
| Opacity / transparency | ✅ | `opacity` (0.05–1.0, default **0.3** — the recommended faint default) |
| Rotation / diagonal | ✅ | `rotation` degrees (−360…360, default **45** = classic diagonal) |
| Position grid (9 presets) | ✅ | `position` enum: center + top/middle/bottom × left/center/right |
| Tiled / mosaic layout | ✅ | `tile` boolean — repeats the rotated stamp in a grid across the page |
| Page range | ✅ | `pages` (`all`, `1,3-5`, open `2-`) — reused from `pdf-page-numbers` |

## Out-of-model (listed, NOT built)

- **Image / logo watermark.** The gizza single-input model gives each tool exactly one primary
  binary input (the PDF); a logo watermark needs a *second* binary input (the image) — the same
  multi-binary-input limit that skiplisted `image-watermark`'s logo-overlay mode. Text watermark
  is the shipped, in-model surface.
- **Drag-to-place / precise X/Y % placement** (Sejda/FunPDF Custom). Interactive canvas
  placement has no analogue in a chat/CLI descriptor; the 9-position grid + rotation covers the
  table-stakes placements. Custom coordinates are a possible future numeric param, deferred.
- **Font-shadow / outline effect** (iLovePDF). Cosmetic; not a placement/legibility table-stake.

## Preset defaults chosen (from competitor consensus)

- opacity **0.3** (competitors recommend 0.15–0.4 for a subtle mark)
- rotation **45°** (the classic diagonal DRAFT/CONFIDENTIAL angle)
- colour **#808080** grey, font **Helvetica**, size **48 pt**, position **center**, `tile` off
- These make the zero-config call `pdf-watermark url=… text=CONFIDENTIAL` already produce the
  expected faint grey diagonal centre stamp.
