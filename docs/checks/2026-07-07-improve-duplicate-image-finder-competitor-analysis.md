# duplicate-image-finder — competitor analysis (2026-07-07)

Tool: scan a batch of images and report exact + near-duplicate pairs above a
similarity threshold, with a suggested keep/delete list. Surfaces: **chat + CLI
only** (array-of-images input + JSON report output → no standalone page, same
shape as `image-collage` / `images-to-pdf` / `gif-from-images`).

## Competitors scanned (paraphrased, no copy/branding reproduced)

1. **Desktop duplicate-photo cleaners** (Duplicate Photo Cleaner, PictureEcho,
   duplicate-photo-finder class) — batch-scan folders, adjustable similarity
   threshold (often a 0–100% slider), exact + visually-similar grouping,
   side-by-side preview, smart auto-selection of which copy to delete, and
   metadata display (dimensions, size, name).
2. **Online duplicate-image finders** (PixDuplicate, pixample, nocodevista) —
   bulk upload of 2+ images, perceptual-hash comparison, similarity scoring on a
   0–100 scale, grouped display with thumbnails, and keep/delete selection.
3. **Perceptual-hash tutorials/libraries** (benhoyt duplicate-image-detection,
   Python `imagehash`, dHash/aHash/pHash write-ups) — compute a compact hash per
   image (a/d/pHash or DCT), compare every pair by Hamming distance, and bucket
   "exact (distance 0–5) / very similar (6–10) / looser (11–20)".

## Table-stakes → decision

| Capability | Competitor norm | Decision |
| --- | --- | --- |
| Bulk input, 2+ images | required | **in-model** — `images` source_list, `minItems: 2` |
| Perceptual-hash near-dup detection | a/d/pHash | **in-model** — hand-rolled 64-bit **dHash** on the pure-Rust `image` crate (no `fast_image_resize`, so it instantiates under `wafer build`; `img_hash`/`image_hasher` were avoided for exactly that reason) |
| Adjustable similarity threshold | 0–100% slider / presets | **in-model** — `threshold` number 0–100, default 90; `similarity% = 100·(1 − hamming/64)` |
| Exact vs near distinction | 100% vs <100% | **in-model** — exact = byte-identical file (FNV-1a-64 + same dims + dHash distance 0); near = perceptually similar |
| Similarity score per pair | 0–100 scale | **in-model** — `similarity` (1 decimal) + raw `distance` (Hamming) per pair |
| Grouped duplicate clusters | grouped display | **in-model** — transitive-closure grouping (union-find) over reported pairs |
| Keep/delete suggestion | auto-select copy | **in-model** — keep = highest resolution (ties → largest file → lowest index); rest are delete candidates; `bytes_reclaimable` roll-up |
| Per-image file info | dims / size / name | **in-model** — width, height, byte size, format, source label, perceptual hash |
| Threshold presets (Exact 100 / Very-similar 95 / Similar 90 / 85) | preset buttons | **partially in-model** — the numeric `threshold` accepts any of these values; the docstring names the 100/90/80 bands. Preset **chips** are a page-only control → N/A here (no page) |
| Thumbnail previews / side-by-side compare | grid of thumbnails | **out-of-model** — this is a headless JSON tool; it returns dims/hash per image, not rendered thumbnails |
| Folder / directory scanning, recursive | scan a drive | **out-of-model** — no filesystem access; caller supplies an explicit list of image sources (url/ref) |
| One-click auto-delete of the duplicates | delete on disk | **out-of-model** — the tool only *suggests* keep/delete; it never mutates files |
| EXIF/metadata-aware dedupe | compare capture time | **out-of-model** — dedupe is pixel/structure-based, not metadata-based |

## Notes / honest limits (also stated in the descriptor)

- Similarity uses a **grayscale structural** hash (dHash), so it is invariant to
  brightness, contrast, resizing and color. Consequence: images that share a
  layout but differ only in color — and flat/solid-color images (a difference
  hash is 0 for any constant image) — can be grouped. Documented as
  "review before deleting". This matches the standard dHash behavior
  (Python `imagehash`, benhoyt's tutorial).
- Per-image caps: 8 MiB on the wire, 24 MP decode guard (clean error, no OOM).
- No copy, branding, screenshots, or trademarks were reproduced from any
  competitor; out-of-model rows are listed, not built.
