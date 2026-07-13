# document-binarize — competitor analysis (2026-07-13)

Tool function: convert a scanned/photographed document image to crisp 1-bit black
text on a white background using classical thresholding (global Otsu + local
adaptive Sauvola/Niblack), so downstream OCR / archival is cleaner.

## Competitor scan (paraphrased — no copy/branding reproduced)

Searched: "online document binarization tool black and white scan adaptive
threshold otsu sauvola". Reviewed the top real tools/references:

1. **Pixlane – Image Thresholding (threshold-segmentation)** — browser-side image
   binarizer. Offers a *global fixed threshold*, *Otsu* (auto global), and *local
   adaptive* methods (Niblack, Sauvola, Wolf, NICK). All processing runs locally in
   the browser (no upload). Exposes a manual threshold value plus the local-method
   window/sensitivity controls.
2. **Rescribe `preproc` (Sauvola CLI / Go package)** — standalone command-line
   binarizer built specifically around the Sauvola algorithm for degraded/historical
   documents; window size and `k` sensitivity are the tunables. Notes that few free
   tools implement Sauvola directly.
3. **Leptonica – grayscale mapping & binarization** — reference C library: grayscale
   conversion then Otsu / adaptive thresholding; documents that adaptive local
   thresholding is the standard answer to uneven page illumination.

Supporting references: Handwriting.guru "Image Binarization Methods for OCR"
(Otsu vs. adaptive trade-offs), arXiv 1201.5227 (local adaptive thresholding), and
the IOP / IJCA document-binarization review papers (Sauvola extends Niblack with a
dynamic-range normalization term aimed at degraded documents).

## Table-stakes parameters / UX (each tagged in-model / out-of-model)

| Capability | Decision |
|---|---|
| Method selector: Otsu (auto global) | in-model — built |
| Method: fixed/manual global threshold (0–255) | in-model — built |
| Method: Sauvola local adaptive (window + k) | in-model — built |
| Method: Niblack local adaptive (window + k) | in-model — built |
| Window/neighbourhood size for local methods | in-model — `window` param |
| Sensitivity `k` for local methods | in-model — `k` param |
| Invert (light text on dark page) | in-model — `invert` param |
| Grayscale luma preprocessing | in-model — always applied before threshold |
| Runs locally / no upload (privacy) | in-model — pure-Rust wasm, runs in chat SW / CLI |
| Wolf & NICK local variants | out-of-model (scope): Sauvola+Niblack cover the same
  degraded-document niche; Wolf/NICK are minor tunings of the same window formula and
  are omitted to keep the control surface focused — could be added later as extra
  `method` enum values. |
| Deskew / perspective-correct before binarizing | out-of-model here — already covered
  by the existing `document-scan` block; compose the two. |
| OCR / searchable text output | out-of-model — needs an OCR engine (see
  `document-text-extract` for embedded-text PDFs); binarize is a preprocessing step. |
| ML / DNN binarization (DE-GAN, DIBCO winners) | out-of-model — needs a neural model,
  not viable in the pure-Rust/ffmpeg gizza runtime. |

## Worked defaults (from the literature)

- Sauvola: `window = 25`, `k = 0.34`, dynamic range `R = 128` (fixed).
- Niblack: `window = 25`, `k = -0.2`.
- Fixed: `threshold = 128`.

Every table-stake above lands in the descriptor or the out-of-model list — none dropped.

## Surface verification notes

- Chat/CLI block shape: `Input::Image` with PNG media output, matching pure-Rust
  image tools such as `blur-image`; there is no standalone `/tools/` page or
  `web/pkg` for this pure image-bytes surface.
- CLI description and invalid-enum path were verified with `gizza describe
  document-binarize` and `gizza tool document-binarize url=https://example.com/x.png
  method=bad`; live URL execution was blocked by the local WRAP network grant in
  the same way as existing `blur-image` URL smoke tests on this machine.
