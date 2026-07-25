# pdf-grayscale — competitor analysis (2026-07-25)

Function: convert a color PDF to grayscale (or pure black & white) to save color
ink when printing and shrink the file. Pure-Rust `lopdf` block; PDF in / PDF out,
so **chat + CLI only, no tool page** (binary-in/binary-out, like pdf-rotate /
pdf-watermark).

## Competitors surveyed (paraphrased — no copy/branding reproduced)

| Tool | Core behavior | Notable options | Notes |
|------|---------------|-----------------|-------|
| Sejda (grayscale-pdf) | Desaturate all page colors to gray | Option to convert **text/vector only and skip images**, or convert everything | Cleanest option set; "skip images" is the standout knob |
| DeftPDF (grayscale) | Upload → grayscale → download | None beyond upload | No-registration, no-watermark, minimal |
| AvePDF (convert-pdf-to-grayscale) | Grayscale a whole PDF | None exposed | Marketing angle = ink saving |
| Dpdf (black-and-white-pdf) | Two output modes | **Grayscale vs pure Black/White (bilevel)** mode toggle | Only competitor exposing a true B/W (1-bit look) mode |
| i2PDF / pdfresizer / pdfhelp | Upload → grayscale → download | None | Emphasize reduced color-ink use and smaller output |

## Table-stakes distilled

- **Desaturate every color on the page** — text, vector fills/strokes, AND raster
  images. This is the baseline every tool ships. → **in-model** (rewrite content
  stream color operators + recolor image XObjects).
- **Grayscale is the default**; a **pure black & white (bilevel)** mode is a real
  secondary offering (Dpdf). → **in-model** as a `mode` enum.
- **Skip images** (convert text/vector only) is Sejda's differentiator — some users
  keep photos in color, or images are what blows up size. → **in-model** as a
  `convert_images` boolean.
- **Whole-document by default**; page selection is expected across the PDF suite
  (every other gizza PDF tool exposes it). → **in-model** as a `pages` range.
- **Ink-saving / smaller-file** is the marketing promise, not a parameter.

## In-model vs out-of-model decisions

In-model (shipped in the descriptor):
- `mode` = `grayscale` (desaturate to gray levels, default) or `black-white`
  (threshold each pixel/color to pure black or white).
- `convert_images` (default true) — recolor raster images too, or leave them color.
- `threshold` (1–254, default 128) — the gray cutoff used only in `black-white` mode.
- `pages` (default `all`) — 1-based range; unselected pages keep their color.

Grayscale math: Rec. 601 luminance `0.299R + 0.587G + 0.114B`; CMYK is first mapped
to RGB (`R=(1−C)(1−K)`, …) then to luminance. Vector operators are rewritten in place
(`rg`→`g`, `RG`→`G`, `k`/`K`→gray, and `sc`/`scn` operand desaturation that keeps the
operand count valid for the active color space). Image XObjects are recolored three
robust ways: **Indexed** palettes are rewritten to gray (encoding-agnostic, touches no
pixels), **DCTDecode (JPEG)** images are decoded/regrayed/re-encoded, and raw / Flate
8-bit DeviceRGB/DeviceCMYK samples are converted per pixel.

Out-of-model (documented as limits, NOT built):
- **True 1-bit/CCITT bilevel image re-encoding** — B/W mode thresholds pixel values
  but keeps 8-bit gray (still prints as pure black/white; not re-encoded to G4 fax).
- **JPEG2000 (JPXDecode) / JBIG2 image recoloring** and images using non-device
  ICC/Lab/Separation spaces with predictors — left untouched to never corrupt output
  (a document limit, surfaced in the tool copy).
- **OCR / re-rasterizing whole pages** (Ghostscript-style flatten) — out of a pure
  `lopdf` model; we transform existing objects rather than re-render pages.

No competitor copy, branding, or trademarks were reproduced; only feature/param
shape was compared.
