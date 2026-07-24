# multi-photo-scan-splitter — competitor analysis (2026-07-22)

**Function.** Take one flatbed scan that holds several loose photos, detect each
photo, auto-straighten (deskew) it, crop it out, and return every photo as its
own image file bundled in a ZIP.

## Competitors surveyed

1. **Photoshop "Crop and Straighten Photos"** (File ▸ Automate) — the classic
   desktop feature. Detects rectangular photos on a scan, straightens each, and
   opens every one in its own document. No size/format knobs; works best with a
   clear gap between photos and a contrasting (usually white) scanner lid.
2. **AutoCropper (autocropper.io/crop-scans)** — browser tool. Auto-detects,
   crops, and straightens every photo on a page; batch-exports each as its own
   file. Accepts JPEG/PNG/TIFF/PDF/HEIC. Emphasises "leave space around each
   photo". Output as individual downloads / archive.
3. **AutoSplitter (autosplitter.com)** — Windows one-time-purchase app. Auto
   detects, straightens, and highlights each photo; lets you nudge boundaries by
   mouse; handles both light and **dark** scanner backgrounds; per-photo output
   files with a naming prefix.
4. **Smart Photo Cropper (smartphotocropper.com)** — computer-vision boundary
   detection for printed photos from scans or phone shots; batch crop to
   separate files.
5. **Stewart Adam's ImageMagick/Fred's `multicrop` workflow** (open-source
   scripting approach) — thresholds the scanner background, finds connected
   photo blobs, deskews via the blob's rotation, and writes each crop. Confirms
   the classical (non-ML) pipeline we use is the standard, reproducible method.

## Table-stakes → model-fit decisions

| Capability | Competitors | In gizza model? | Decision |
|---|---|---|---|
| Detect multiple photos on one scan | all | ✅ classical background-segmentation + connected components (pure `image`) | **built** (core) |
| Auto-straighten / deskew each photo | Photoshop, AutoCropper, AutoSplitter | ✅ min-area-rectangle of each blob → rotate | **built** (`straighten`, default on) |
| Crop each into a separate file | all | ✅ per-blob crop → ZIP of images (same shape as `spritesheet-slice`) | **built** (ZIP output) |
| Light **and** dark scanner backgrounds | AutoSplitter | ✅ sample the border; `auto`/`white`/`black` | **built** (`background`) |
| Ignore dust / specks / tiny fragments | all (implicit) | ✅ minimum-size filter | **built** (`min_size`) |
| Trim residual scanner-bed bleed at the edges | AutoSplitter (manual nudge) | ✅ inward inset | **built** (`edge_trim`) |
| Output format choice | AutoCropper (JPEG/PNG/TIFF) | ✅ png/jpeg/webp/bmp | **built** (`format`) |
| Custom filename prefix | AutoSplitter | ✅ | **built** (`prefix`) |
| Cap number of photos | — | ✅ safety cap | **built** (`max_photos`) |
| Manual boundary nudging (mouse) | AutoSplitter | ❌ interactive UI, no batch surface | out-of-model (documented) |
| HEIC / PDF / multi-page TIFF input | AutoCropper | ❌ no HEIC/PDF decoder in the pure-Rust stack | out-of-model (accept PNG/JPEG/WebP/BMP/GIF) |
| ML "which way is up" (face/sky orientation) | AI croppers | ❌ needs a model; gizza is pure-Rust + ffmpeg | out-of-model (deskews to the nearest ±45°; user rotates if upside-down) |
| Perspective de-warp of a phone photo | Smart Photo Cropper | ➖ covered by the sibling `document-scan` tool | out-of-scope (different tool) |

## UX / control patterns adopted

- **Preset chips** (`[[example]]`) for the common jobs: white-background scan,
  dark-lid scan, straighten-off, JPEG export.
- `background` renders as a labelled `<select>` (Auto / White lid / Black lid).
- `straighten` is a checkbox, default **on** (every competitor straightens by
  default).
- `min_size` and `edge_trim` are plain pixel number fields with placeholders.

## Notes / honesty

- Detection is classical (threshold + connected components + min-area rectangle),
  not ML — robust when photos have a **clear gap** and the scanner background
  contrasts with them, exactly the workflow every competitor documents
  ("leave space around each photo"). Touching/overlapping photos merge into one
  region; low-contrast borders may be missed — both are stated on the page.
- The scan is analysed at up to 1600 px on its long side to stay within the
  sandbox memory budget; crops are produced from that working copy.
- No competitor copy, branding, or trademarks were reproduced — only the public
  capability set was compared.
