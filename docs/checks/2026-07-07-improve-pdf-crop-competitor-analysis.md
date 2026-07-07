# pdf-crop — competitor analysis (2026-07-07)

Tool: **pdf-crop** — "Crop page margins or set a crop box on PDF pages (uniform or per-page)."
Type: pure PDF-bytes transform via `lopdf` → chat + CLI only (PDF bytes have no page render
surface, same as every other gizza PDF tool: pdf-rotate, pdf-split, pdf-delete-pages, …).

## Competitors scanned (paraphrased — no copy/branding reproduced)

Searched "crop PDF online tool margins ..." and skimmed the top real tools:

1. **WuTools — Crop PDF.** Per-side margins Top/Bottom/Left/Right. Units selectable:
   Points (pt), Millimeters (mm), Inches (in), with 72 pt = 1 in. Page selection: all / odd /
   even / custom range (e.g. `1-5`). Visual drag handles + preview. Quick presets: Reset,
   Auto-detect, 10pt/25pt/50pt uniform, and crop-to-size (A4/A5/Letter/Legal).
2. **PDF24 — Crop PDF.** Per-side Left/Top/Right/Bottom in mm. "Box Crop" / "Trim Box" auto
   options. Visual mode. Each page margin set individually.
3. **DeftPDF — Crop PDF.** Per-side Top/Bottom/Left/Right in inches. Two modes: crop whole
   document (same measurements) or crop pages individually with different settings per page.
   Auto-crop whitespace. Click-and-drag on a thumbnail preview.

(Also glanced: Sejda "auto-trim white margins one click", ShowPro "PDF Crop Margins" per-side
mm + extend-margins, iLovePDF, pdfresizer, Smallpdf — same table-stakes cluster.)

## Table-stakes → decision (every one tagged; none dropped silently)

| Feature | Competitors | Fit | Decision |
|---|---|---|---|
| Per-side margins top/bottom/left/right | all | in-model | `top`/`bottom`/`left`/`right` number params (≥0) |
| Uniform crop (same all sides) | WuTools/ShowPro presets | in-model | set the four params equal; no page → no preset chips needed |
| Units pt / mm / in | WuTools, PDF24(mm), DeftPDF(in) | in-model | `unit` enum (pt\|mm\|in), 72pt=1in, 1mm=72/25.4pt |
| Page selection all / odd / even / range | WuTools, PDF24, DeftPDF | in-model | `pages` string: `all`\|`odd`\|`even`\|`1,3-5` |
| "Set a crop box" (write the PDF /CropBox) | all (that's the output) | in-model | the tool writes each selected page's `/CropBox` (inset from the current visible box) |
| Auto-detect / trim whitespace to content | all 5 | **out-of-model** | needs rasterizing each page to find ink bounds; no pure-Rust wasm PDF renderer here |
| Visual drag-to-crop preview + thumbnails | all 5 | **out-of-model** | PDF-bytes tools have no visual page surface in gizza (chat+CLI only) |
| Crop-to-standard-size (A4/Letter/…) | WuTools, ShowPro | out-of-scope | distinct "fit page to size" op, not margin cropping — belongs in its own tool |
| Extend margins / add whitespace (negative crop) | ShowPro, WuTools | out-of-scope | the inverse operation (grow the page); a crop tool crops. A future `add-pdf-margins` tool |

### Notes on the out-of-model / out-of-scope calls (feasibility spikes)

- **Auto-detect whitespace**: genuinely needs a rasterizer (render page → scan for non-white
  pixels → compute bounding box). gizza has no pure-Rust/wasm PDF page renderer, so this is not
  a filtergraph-away capability — correctly out-of-model.
- **Crop-to-size / extend-margins**: both are *feasible* geometry with lopdf (write a centered
  `/CropBox`, or grow the `/MediaBox`), but each is a different operation from "crop the margins
  off". Bundling grow+crop into one tool is the dual-purpose smell we avoid — listed here so the
  next builder can spin them off as their own tools rather than silently dropping them.

### Why no explicit "x0,y0,x1,y1 box" text field

None of the five competitors exposes raw absolute crop-box coordinates as a typed field — every
one uses per-side margins (+ visual drag, which we can't do headlessly). Absolute coords also
require the user to know each page's MediaBox origin/size (which varies per page), so it is not a
table-stake. "Set a crop box" is honored as the *result*: the tool writes each page's `/CropBox`.

## Final descriptor

`Input::Document` (url⊕ref) + `top`, `bottom`, `left`, `right` (number ≥0, default 0),
`unit` (enum pt|mm|in, default pt), `pages` (string, default `all`; accepts `odd`/`even`/ranges).
Requires at least one non-zero side (no silent no-op). Non-destructive: writes `/CropBox`, leaves
page content streams untouched, so the crop is reversible by resetting the box.
