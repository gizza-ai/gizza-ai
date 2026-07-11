# Competitor analysis — image-document-shadow-remove (2026-07-11)

Function: remove cast shadows / uneven lighting from phone photos of documents to
produce clean, flat, near-white pages. All findings paraphrased; no competitor copy,
branding, or trademarks reproduced.

## Competitors scanned (top real tools for the *document* shadow-removal function)

1. **Online2PDF — "remove shadows in JPG/PDF scans"** — one-click: uploads a scan/photo
   and transforms the shadowed background to white. Exposes almost no shadow-specific
   controls (a general options panel covers compression/OCR/layout, not shadow tuning).
   Input JPG/photo → output PDF (its main product is the PDF wrapper). Page caps: 500
   without OCR, 100 with OCR. OCR in 23+ languages. No before/after examples or algorithm
   detail on the page.
2. **PicWish — AI shadow remover (document mode)** — automatic AI detection; one of four
   documented modes is "document clarity" (study material, IDs, business docs). Upload →
   auto-remove → download. No brightness/contrast/threshold sliders or output-format
   controls documented; prioritises zero-config simplicity. Also does portrait/product
   shadow removal (a different, segmentation-based function).
3. **pdfFiller / Pokecut — "remove shadow in image"** — brush/manual-erase UX: the user
   paints over the shadow region and it is cleared; aimed at both scanned docs and product
   photos. Interactive, layer/brush based rather than a single algorithmic pass.

## Table-stakes → where each lands

| Table-stake (from the scan)                     | In/out of model | Landing |
|-------------------------------------------------|-----------------|---------|
| Automatic shadow / uneven-lighting removal → clean near-white page | in-model | **descriptor (core illumination-normalization pass)** |
| Colour vs grayscale vs pure black-&-white output | in-model | **descriptor `mode` = color/grayscale/blackwhite** |
| Control over how hard the background whitens     | in-model | **descriptor `whiteness` 0–100** |
| Preserve coloured ink / highlights while whitening paper | in-model | **descriptor `mode=color` (division-normalisation keeps hue)** |
| PDF output / multi-page batch                    | out-of-model here | listed below (chain the existing `images-to-pdf` tool) |
| OCR / searchable text                            | out-of-model | listed below (needs an ML text model) |
| Manual brush-to-erase a shadow region            | out-of-model | listed below (interactive canvas UX, not a query→result tool) |
| AI product/portrait cast-shadow removal          | out-of-model (different function) | listed below (needs subject segmentation ML) |
| Auto perspective crop / dewarp                   | separate tool   | already shipped as `document-scan`; not duplicated here |

## Design decisions (in-model, shipped)

- **Algorithm** (pure Rust, `image` crate; no ML, runs on every backend incl. chat SW):
  per-channel estimate the local "paper white" via a downscaled max-filter + blur
  (large low-pass that erases text/ink and keeps the illumination envelope), then
  **division-normalise** `out = clamp(pixel / background * 255)`. Because a shadowed
  region has a correspondingly darker local background, the division flattens the shadow
  gradient and lifts the page to uniform white while preserving strokes. `whiteness`
  raises the white-point (snaps near-white paper to pure 255); `blackwhite` finishes with
  an Otsu threshold for crisp forms.
- **Params:** `mode` (`Param::enumv` color/grayscale/blackwhite, default color),
  `whiteness` (0–100, default 55). Two controls mirror the "few knobs, mostly automatic"
  reality of the competitors while still giving the colour-treatment + aggressiveness that
  document users actually reach for.
- **Surface:** image-bytes in → PNG out ⇒ **chat + CLI, no standalone page** (the page
  file-input path is ffmpeg-only; matches image-to-sketch / image-pixelate-censor).

## Out-of-model (considered, NOT built)

- PDF output & multi-page batch — chain the existing `images-to-pdf` block after this one.
- OCR / searchable-text export — needs an ML text-recognition model.
- Manual brush / lasso shadow erase — interactive canvas UX, outside the query→result model.
- AI product/portrait cast-shadow removal — needs subject segmentation (ML); a different
  function from document illumination flattening.
- Perspective dewarp / auto-crop of the page quadrilateral — already provided by the
  separate `document-scan` tool; intentionally not duplicated.
