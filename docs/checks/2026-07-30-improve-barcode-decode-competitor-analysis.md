# barcode-decode — competitor analysis (2026-07-30)

Tool: reads 1D barcodes from an image and returns the encoded value(s).
Engine spike: `rxing` 0.7.1 (pure-Rust ZXing port) + `image` — verified natively decoding
EAN-13, UPC-A, Code 128 and Code 39 from generated bitmaps (paraphrased notes only; no
competitor copy/branding reproduced).

## Competitors scanned (top 3)

1. **Scanly (scanly.co/barcode-scanner)** — decodes eight 1D symbologies (EAN-13, EAN-8,
   UPC-A, UPC-E, Code 128, Code 39, Codabar, ITF). Input by upload, clipboard paste, or live
   camera; accepts JPG/PNG/WebP/GIF/BMP/AVIF; runs locally. Output shows the detected format,
   the decoded value with a copy button, and a web-search link. No user-facing settings
   (no format pre-select, no "try harder", no multi-count).
2. **ReadBarcode (readbarcode.com/barcode-reader)** — auto-detects a very broad set (UPC-A/E,
   EAN-8/13, Code 39/93/128, ITF, ITF-14, Codabar, RSS, plus 2D). Input by camera, upload, or
   image URL. Output shows decoded value, format, and a scan timestamp. No configurable
   options. Explicitly warns against blurry/low-light images, reflections, cropped quiet
   zones, heavy JPEG compression.
3. **mate.tools (mate.tools/barcode-reader)** — QR + all major 1D (EAN-13/8, UPC-A/E, Code
   128/39/93, Codabar, ITF). Accepts JPG/PNG/GIF/WEBP up to 10 MB. Reads **one** barcode at a
   time (most prominent). Tips: well-lit, in-focus, high-contrast, higher resolution on
   failure. No settings.

## Table-stakes → decision (in-model / out-of-model)

| Capability | Decision |
|---|---|
| Decode 1D barcode from image → value | **in-model** — core output |
| Detected format name in output | **in-model** — each result carries `format` |
| Multiple image formats (PNG/JPEG/GIF/WebP/BMP) | **in-model** — `image` crate features |
| Image-URL input | **in-model** — `Input::Image` accepts `url` ⊕ `ref` |
| Auto-detect symbology | **in-model** — `format=auto` default (broad 1D set) |
| Restrict to one symbology (reduce misreads) | **in-model** — `format` enum param |
| "Try harder" thorough scan | **in-model** — `try_harder` boolean (default on) |
| Decode **multiple** barcodes in one image | **in-model** — returns every 1D code found (beats mate.tools' one-at-a-time) |
| Live camera scanning | **out-of-model** — no camera in a pure decode block |
| Drag-drop / clipboard-paste UX | out-of-model (page file-input handles upload; not a descriptor param) |
| Web-search link for the value | out-of-model (external/branding — not built) |
| 2D codes (QR/DataMatrix/Aztec/PDF417) | out-of-model here — QR already ships as `qr-decode`; this tool is 1D-only |

## UX control patterns matched
- `format` is a fixed-choice `<select>` (`Param::enumv`): `auto`, `ean-13`, `ean-8`,
  `upc-a`, `code-128`, `code-39`.
- `try_harder` renders as a checkbox (boolean), default checked.
- Example preset chips on the page for the common image-URL + format combos.

## Descriptor (final)
- input: image (`url` ⊕ `ref`), formats png/jpeg/gif/webp/bmp
- `format`: enum, default `auto`
- `try_harder`: boolean, default `true`
- output: `{ count, barcodes: [{ format, text }] }` — every 1D code found, in detection order

Limits stated on the page: 1D symbologies only (use `qr-decode` for QR); needs a clear,
high-contrast image with intact quiet zones; large images are rejected with an actionable
size error (64 MiB sandbox).
