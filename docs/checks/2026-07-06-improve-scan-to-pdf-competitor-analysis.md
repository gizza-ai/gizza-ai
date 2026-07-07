# scan-to-pdf — competitor analysis (2026-07-06)

Tool: turn one or more phone photos of documents into a cleaned, deskewed,
high-contrast multi-page PDF scan. Classified **pure** (pure-Rust `image` +
`lopdf`, no ffmpeg, no ML). PDF output ⇒ **no standalone page** (the page driver
only renders image/video/audio/text; every `*-to-pdf` tool in this repo is
chat+CLI-only — see `images-to-pdf`, `markdown-to-pdf`, `svg-to-pdf`). Surfaces:
chat + CLI.

## Competitors skimmed (paraphrased — no copy/branding reproduced)

1. **CamScanner** (mobile scanner). Five enhancement modes: Original, Lighten,
   Magic Color (default), Grayscale, Black & White. Auto-crop the capture +
   auto-straighten. Magic Color is the everyday default.
2. **Scanner Pro (Readdle)**. Five modes: Black & White (highlights/contrasts
   text), Color (adds brightness+contrast to a colour scan), Photo (preserves
   all colours), Grayscale (keeps brightness/contrast, drops colour), Auto Color
   (picks the best filter per document). Rotate + edit.
3. **Omnvert Document Scanner** (online). Auto-crop via 4-corner quadrilateral
   edge detection; deskew/perspective correction via a homography matrix;
   paper-whitening; four modes Color / Magic Color (default, whiten+sharpen) /
   Grayscale (perception-weighted) / B&W (adaptive threshold, ~31×31 neighbour
   mean). Page sizes A4 / US Letter / Original. Multi-page reorder → single PDF.
4. (cross-check) **iScanner / Sejda / AvePDF** — B&W + Grayscale filters,
   straighten/deskew, despeckle (remove blemishes/speckles), brightness.

## Table-stakes → decision (every one lands in the descriptor OR is listed here)

| Capability | Fit | Decision |
|---|---|---|
| Multiple enhancement modes (magic/grayscale/blackwhite/color) | in-model | `mode` enum, default `magic` |
| Magic Color (paper-whiten + contrast + saturation boost) | in-model | `mode=magic`: white-point normalize + contrast S-curve + saturation lift |
| Grayscale (Rec.601 perception-weighted luma) | in-model | `mode=grayscale`, embedded DeviceGray |
| Black & White adaptive threshold (local-mean, office-scan look) | in-model | `mode=blackwhite`: integral-image local-mean threshold + 3×3 median despeckle |
| Contrast control | in-model | `contrast` number (0.5–3.0, default 1.0) |
| Brightness control | in-model | `brightness` number (−100..100, default 0; also biases the B&W threshold) |
| Auto-deskew / straighten small tilt | in-model | `deskew` bool (default true): projection-profile skew detection + bilinear rotation |
| Manual rotate (phone orientation) | in-model | `rotate` enum 0/90/180/270 |
| Despeckle B&W output | in-model | folded into `blackwhite` (3×3 median), not a separate param |
| Page size A4 / Letter / Original | in-model | `page_size` enum fit/a4/letter (default fit) |
| Multi-page (several photos → one PDF, in order) | in-model | `images` source_list, min 1 |
| **Auto-crop / perspective de-warp (4-corner homography)** | **out-of-model** | needs robust CV edge+contour detection + homography warp — not reliably doable pure-Rust without OpenCV. Deskew (rotation) + manual rotate cover the common tilt/orientation cases; full corner-crop is **listed, not built**. |
| **OCR / searchable-text PDF** | **out-of-model** | needs an OCR model — listed, not built. |

## UX control patterns competitors ship (recorded even though this is no-page)

Filter chips per mode, brightness/contrast sliders, rotate buttons, crop
handles. No standalone page for a PDF-output tool, so these render as the
chat/CLI params above; the descriptor exposes the mode/rotate/page-size choices
as enums and brightness/contrast as bounded numbers so an LLM or CLI user has
the same knobs.

## Worked defaults chosen

`mode=magic`, `deskew=true`, `rotate=0`, `contrast=1.0`, `brightness=0`,
`page_size=fit` — mirrors the everyday "Magic Color + auto-straighten, keep
original page aspect" default of the mobile scanners.
