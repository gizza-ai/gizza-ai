# images-to-pdf — competitor analysis (2026-06-20)

Twelfth `/create-next-tool` backlog pick (image-watermark was skiplisted before it
— its text mode dups add-text-to-image and its logo mode needs 2 inputs). Pure-Rust
tool (lopdf + image + flate2). `Input::None` + a `source_list` of images (like
merge-pdf). Surfaces: chat + CLI (no page — array input + pure-Rust PDF bytes).
Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| jpg2pdf / png2pdf | combine many images into one PDF; JPG/PNG/BMP/TIFF/SVG | capabilities |
| Smallpdf | reorder, page size, orientation, margins | capabilities / UX |
| XConvert | paper-size choice incl. "Original" (page = image size); reorder; mixed formats incl. HEIC/RAW | capabilities |
| ImageToPDF / Adobe | drag to reorder; mix formats in one batch | UX |

## Gap diff vs our tool
Our tool: combine ≥1 image (PNG/JPEG/WebP/GIF/BMP, mixed) into one PDF, one image
per page in the given order, each page sized to its image ("Original" size — the
option XConvert highlights). Each image is decoded to RGB and embedded as a
Flate-compressed image XObject.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **Page-size presets** (A4 / US Letter / etc.) with the image scaled-to-fit and
  centered + optional margin — we currently always use the image's own size. A
  `page_size` + `margin` param is a clean future add (pure layout math).
- **Orientation** (force portrait/landscape) — pairs with page-size presets.
- **JPEG pass-through** (embed JPEG bytes via DCTDecode instead of re-encoding to
  Flate RGB) — smaller output for photo inputs; an optimization.

**Out-of-model:** drag-to-reorder thumbnails UI (order is the array order),
HEIC/RAW decode (heavy/seldom-supported codecs), batch of separate PDFs.

## Tested
unit (5: 1 image→1 page, 3 images→3 pages, MediaBox matches image size, empty
error, bad-image error) + drift-guard · `wafer build` validates the block
(lopdf+image+flate2 → wasm32-wasip1; pure-Rust so also works in the chat SW) ·
CLI on real public images (1 image → 1-page PDF; 2 images → a verified 2-page
%PDF) + non-image MIME-guard error. No page surface.

> Original work only — no competitor copy, branding, or trademarks copied.
