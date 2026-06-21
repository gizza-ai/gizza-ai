# strip-exif — competitor analysis (2026-06-21)

Tool: `blocks/strip-exif` — remove all metadata (EXIF, GPS, XMP, IPTC, comments)
from a JPEG or PNG and return a clean copy **without re-encoding the pixels**.

Surfaces built: **chat (LLM API)** + **CLI**. No standalone page — image-bytes
output has no page render mode in gizza's page driver (the documented no-page
image-output pattern, like `image-collage`). All three of: drift-guard schema
test, `wafer build` validation/instantiation, and live CLI runs against real
EXIF/GPS-bearing photos pass.

## Competitors surveyed (paraphrased — no copy/branding reused)

1. **ExifCleaner** (exifcleaner.com / GitHub szTheory/exifcleaner) — cross-platform
   desktop GUI. Strips EXIF from photos, videos, PDFs. Headline feature: **batch /
   drag-multiple**, folder recursion, "metadata inspection to see exactly what was
   removed," preserve-orientation, save-as-copy, xattr removal, 25 locales. Desktop
   app, not browser-local.
2. **ImageOptim** (Mac) — strips metadata **plus** lossless file-size optimization;
   **selective** removal (choose which metadata classes to drop) rather than
   all-or-nothing; quality/optimization tuning.
3. **VereXif** (verexif.com) — browser EXIF viewer **and** remover; view-then-strip
   flow in one page; JPEG-focused.
4. **MetaClean** (metaclean.app) — **client-side, files never leave the browser**;
   JPEG/PNG/HEIC/WebP; markets the privacy/no-upload architecture as the key
   differentiator.
5. **metadataview.com/remove** & **exifremover.com** — browser tools; JPG/PNG/WEBP/
   HEIC/TIFF/GIF; "scrub in place without re-encoding"; searchable metadata table
   with **selective** per-item removal before stripping.

## Gap diff vs gizza strip-exif

| Dimension | Competitor capability | gizza strip-exif | Verdict |
|---|---|---|---|
| Lossless (no pixel re-encode) | MetaClean / metadataview claim in-place scrub | **Yes** — `img-parts` rewrites only the segment/chunk list; IDAT/scan bytes untouched | **At parity / strength** |
| Privacy (local, no upload) | MetaClean's headline | **Yes** — pure-Rust wasm, runs locally incl. the chat Service Worker, no server | **At parity / strength** |
| "See what was removed" | ExifCleaner inspection; viewer+remover combos | Returns a report: format, byte delta, **segments_removed count**, **had_exif** flag | **In-model, shipped (count-level)**; per-tag inspection is covered by the sibling `image-metadata-viewer` tool — chained, not duplicated |
| Formats: JPEG/PNG | All | **Yes (both)** | **At parity** |
| Formats: WebP | MetaClean, metadataview | No — `img-parts` 0.3 has no RIFF/WebP container support | **Out-of-model (library limit)**; would need a new container parser |
| Formats: HEIC / TIFF / GIF | Some browser tools | No | **Out-of-model**; HEIC needs licensed decode; GIF/TIFF rarely carry EXIF |
| Selective per-class removal | ImageOptim, metadataview | No — strips all personal metadata, **keeps** colour/render data (JFIF APP0, ICC APP2, gAMA/sRGB/iCCP) by design | **In-model but intentionally not built** — privacy tools want "remove everything personal" by default; keeping colour profiles avoids visible colour shifts, which is the safer default |
| Batch / folder | ExifCleaner | No — single image per call | **Out-of-model** — single-input descriptor + page driver take one source (matches the rest of gizza's image tools) |
| Video / PDF metadata | ExifCleaner, exifremover | No (this tool is images only) | **Out of scope** — separate tools |

## What was built / kept

- **JPEG**: drop APP1 (EXIF, GPS, XMP), APP13 (Photoshop/IPTC), COM (comments);
  **keep** APP0 (JFIF density) + APP2 (ICC colour profile) so colours/aspect render.
- **PNG**: drop tEXt/zTXt/iTXt (text), eXIf, tIME, dSIG; **keep** all critical
  chunks (IHDR/PLTE/IDAT/IEND) + colour/render ancillaries (gAMA/cHRM/sRGB/iCCP/
  bKGD/pHYs/tRNS/sBIT).
- **Report** for the LLM: format, input/output bytes, removed bytes, segment count,
  had_exif — so chat/CLI users see what was scrubbed (the inspection idea, at the
  count level; full per-tag listing lives in `image-metadata-viewer`).
- **No re-encode**: only the segment/chunk vector is filtered; pixel/scan data is
  byte-identical, so there is **zero quality loss** — a genuine differentiator vs
  any tool that re-saves through an encoder.

## Verification (live)

- CLI on `exif-samples/.../gps/DSCN0010.jpg` (GPS-tagged): "stripped 2 segments
  (including EXIF/GPS), 161713 → 146420 bytes"; output re-checked — no `Exif\0\0`,
  no APP1, no "GPS", valid SOI/EOI.
- CLI on `Pillow/.../exif.png`: "stripped 8 segments (incl. EXIF), 179336 → 178951";
  output re-checked — no eXIf/tEXt/iTXt/zTXt, IHDR/IDAT/IEND intact.
- CLI on a clean PNG: "stripped 0 segments", had_exif=false (correct no-op).
- Non-image input is rejected (content-type guard + core `unsupported` error).

## Out-of-model / considered-not-built (summary)

WebP/HEIC/TIFF/GIF support, selective per-class removal, batch/folder processing,
video & PDF metadata. None fit gizza's single-input, pure-Rust+ffmpeg, no-server
model with the current `img-parts` 0.3 container support; recorded here, not forced in.
