# image-shrink-for-sharing — competitor analysis (2026-07-06)

One-step "shrink an image for messaging / upload": downscale to a max dimension,
strip metadata, and re-encode at a chosen quality (optionally converting format),
in a single browser-local ffmpeg pass. Sits between the existing single-purpose
tools `image-resize` (scale only), `image-compress` (quality only), and
`strip-exif` (metadata only) — its value is doing all three at once.

## Competitors scanned (paraphrased — no copy/branding reproduced)

1. **shrink.media** — upload → pick a pixel size → download. Formats PNG/JPG/JPEG/WEBP/HEIC,
   caps ~5000×5000 px / 25 MB. Offers many fixed KB/MB size targets as separate variants;
   before/after comparison view. Does not surface a quality slider or explicit metadata toggle.
2. **img2go compress-image** — 7 modes: Recommended (balanced default), Extreme, Lossless (PNG),
   **Target file size** (100 KB / 200 KB / 1 MB), Percentage-of-original, **Custom quality 0–100
   slider**, **Strip metadata** (EXIF/GPS). Formats JPG/PNG/GIF/BMP/TIFF/WebP. Batch + cloud import.
3. **simpleimageresizer** — resize by percentage or W×H, keep-aspect-ratio toggle, fit modes,
   social-platform presets (Instagram/Facebook/LinkedIn). Formats JPEG/PNG/WEBP/HEIC/BMP/GIF.
   Batch (3 free). Quality/format controls not prominent.

(Squoosh and imagecompressor.com corroborate: quality slider + resize + format choice +
browser-local processing are the consistent table stakes.)

## Table-stakes → decision (every one tagged; none dropped silently)

| Table stake | Decision |
|---|---|
| Downscale to a max pixel size, aspect kept, no upscale | **IN** — `max_dimension` field (0 = keep original) |
| Quality control 1–100 | **IN** — `quality` param, rendered as a **slider** (default 80) |
| Strip metadata (EXIF/GPS) | **IN** — `strip_metadata` boolean, default **true** (tool premise) |
| Output format choice (keep/JPEG/PNG/WebP) | **IN** — `format` enumv, default `keep` |
| Keep aspect ratio | **IN** — always preserved; never upscales |
| Social / messaging presets | **IN** — `[[example]]` preset chips (messaging, story, email, keep-size) |
| Target file size in KB (compress-to-200 KB) | **OUT-OF-MODEL** — needs an iterative quality search; a single `build_argv`→one ffmpeg exec can't binary-search size. Same deferral image-compress made. Stated on the page. |
| Batch / multi-file | **OUT-OF-MODEL** — the page takes one file upload. |
| Before/after comparison slider | **CONSIDERED, REJECTED** — the shared generated page renders the result + a download link; a bespoke split-compare view is out of the shared page's scope and would be a per-tool hack. |
| Cloud import (Drive/Dropbox) | **OUT-OF-MODEL** — no backend / accounts (browser-local by design). |
| HEIC input | **OUT-OF-MODEL** — the browser ffmpeg build does not reliably decode HEIC; inputs restricted to JPEG/PNG/WebP, stated on the page. |

## UX control patterns matched

- **Quality slider** (`kind = "slider"`, 1–100, step 1) — matches img2go's custom-quality slider.
- **Format `<select>`** with friendly `[input.labels]` — matches format-choice competitors.
- **Preset chips** (`[[example]]`) for messaging/story/email/keep-size — the declarative answer
  to competitors' social-preset buttons and quick size targets.
- **Strip-metadata checkbox**, default on — matches img2go's metadata mode, made the default.

## Spike notes (feasibility before tagging)

`scale='min(N,iw)':'min(N,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2`
verified on ffmpeg 6.1: caps the longest side to N, preserves aspect ratio, never upscales
(101×100 → 100×100), rounds output to even dimensions so JPEG (yuvj420p) encodes cleanly even
at tiny targets (max 3 → 2×2). `-map_metadata -1` strips metadata; per-format encoder flags
(`-q:v` / `-compression_level` / `-quality`) mirror image-compress. All in one ffmpeg pass.
