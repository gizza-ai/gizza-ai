# document-scan — competitor analysis (2026-07-07)

Tool function: take a photo of a document and produce a flat, cropped "scan" —
detect the page's four corners, perspective-correct (dewarp) it to a rectangle,
and tonally clean it up. Paraphrased notes only; no competitor copy/branding is
reproduced.

## Competitors scanned

1. **PerspectiveFix** (browser, client-side) — you mark the four corners of the
   document by hand; a projective warp "pulls" the marked quad flat. Manual-corner
   perspective correction, 100% in-browser, no upload.
2. **Omnvert Document Scanner** — runs a fast edge-detection pass on a *downscaled*
   copy, looking for the strongest contrast boundary that forms a quadrilateral;
   works well when there is clear contrast between page and background. Four
   stages: capture → edge detection → perspective correction → tonal enhancement.
   Classical CV (no ML claimed).
3. **Camera Scanner Online** — contour detection → picks the four corners → runs a
   perspective warp to flatten the page as if shot straight on. Classical CV.
4. **Pixelcut Document Straightener** — AI auto-detects edges and squares the
   perspective. ML-based auto-detection.
5. **PassportPhoto.online Document Scanner** — Harris corner detection + homography
   transform to produce a rectangular document. Classical corner detection + ML-ish
   refinement.

## Table-stakes → in-model / out-of-model

| Capability | Decision | Where it lands |
|---|---|---|
| Perspective / homography warp of a 4-corner quad to a flat rectangle | **in-model** | core `warp_perspective` (8-DOF homography, bilinear inverse sampling) |
| Manual four-corner input (PerspectiveFix) | **in-model** | `corners` param (`x0,y0…x3,y3`, TL,TR,BR,BL) |
| Automatic edge/corner detection, classical (Omnvert / Camera-Scanner: contrast-boundary largest-quadrilateral on a downscaled copy) | **in-model (best-effort)** | core `detect_corners` (Otsu brightness split → extreme-corner quad); assumes the page is lighter than its background + fully in frame; errors clearly instead of emitting garbage when it can't find a confident quad |
| Tonal enhancement / paper whitening (Omnvert stage 4) | **in-model** | `mode` = magic / grayscale / blackwhite / color |
| Output page proportion (fit / A4 / Letter / square) | **in-model** | `output` = auto / a4 / letter / square |
| Orientation fix (rotate 90/180/270) | **in-model** | `rotate` param |
| White border / margin | **in-model** | `margin` param (0–25%) |
| **ML/AI edge detection** (Pixelcut; sub-pixel Harris refinement in PassportPhoto) — robust on cluttered / low-contrast / same-colour backgrounds | **out-of-model** | needs a trained model; gizza is pure-Rust + no ML. Classical detection covers the clear-contrast case; ML robustness is listed, not built. |
| **OCR / searchable text** | **out-of-model** | no OCR engine; this tool produces an image, not text (same boundary scan-to-pdf documents) |
| **Interactive drag-the-corners UI** | **out-of-model (surface)** | this is a chat+CLI image tool (image-bytes output → no standalone page, like blur-image / scan-to-pdf); the page driver has no file→wasm image surface. Corners are supplied as coordinates. |

## UX controls competitors ship

- Drag-handle corner picker on the uploaded image (interactive) — not expressible
  on gizza's declarative form pages; corners are passed as coordinates instead.
- Enhancement mode toggle (color / gray / B&W / "magic") — mirrored as the `mode`
  enum.
- Auto-detect toggle — mirrored implicitly: omit `corners` = auto, pass them =
  manual.

## Decision

Build as a pure-Rust single-image tool (chat + CLI, no page — the established
shape for every image→image gizza block). Auto-detect is the default headline
behaviour with a graceful failure + explicit-corners fallback; ML auto-detection
and OCR are the only genuinely out-of-model gaps. Distinct from `scan-to-pdf`
(which does small-angle deskew + PDF output and explicitly does NOT do 4-corner
perspective de-warp) and from `image-crop` / `pdf-crop` (axis-aligned rectangle
crops, no perspective).

Sources (paraphrased, not quoted): PerspectiveFix; Pixelcut; Omnvert; Camera
Scanner Online; PassportPhoto.online.
