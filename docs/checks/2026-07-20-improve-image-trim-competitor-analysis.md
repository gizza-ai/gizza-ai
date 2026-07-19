# image-trim — competitor analysis (2026-07-20)

New-tool build scan (done BEFORE implementing). Sources skimmed (paraphrased only — no
competitor copy/branding reproduced): ImageTools.org "Trim Images", Pico Image "Trim",
Trimmy!, plus ImageMagick `-trim` semantics as the de-facto CLI standard.

## What competitors ship

- **ImageTools.org trim** — corner-pixel background detection (transparent or white),
  single aggressiveness parameter: the percentage of background pixels a row/column may
  contain and still be trimmed (numeric input). Before/after worked example. States the
  corner-sampling limitation explicitly.
- **Pico Image trim** — two detection modes (transparent-alpha, background color), a
  0–255 tolerance slider (0 = exact match; warns high values eat into the subject),
  auto-sampled or manually picked border color, pixel padding after trim (default 0),
  an "edge connected" checkbox, and an optional ML background-removal preprocessing
  step. Client-side processing; JPG/PNG/GIF/WEBP/SVG.
- **Trimmy!** — one-click auto trim (no documented parameters), plus unrelated batch
  crop/resize/optimize/convert features.
- **ImageMagick `-trim`** — background = corner color, `-fuzz N%` tolerance,
  `trim:percent-background` define for noisy edges, error when the whole image is
  background.

## Table stakes → descriptor mapping

| Capability | Tag | Where it landed |
|---|---|---|
| Trim transparent (alpha) padding | in-model | `mode=transparent` (also auto-detected from transparent corners in `mode=auto`) |
| Trim solid-color border auto-detected from corners | in-model | `mode=auto` (majority vote of the 4 corner pixels) |
| User-specified border color | in-model | `mode=color` + `color` hex param (`#rgb`/`#rrggbb`) |
| Tolerance / fuzz (0–255, anti-aliasing + JPEG artifacts) | in-model | `tolerance` integer 0–255, default 16 |
| Padding kept around content after trim | in-model | `padding` integer 0–500 px (kept from the ORIGINAL border, clamped to the image edges — no synthetic pixels) |
| Row/col background-percentage aggressiveness (noisy edges, stray pixels) | in-model | `background_percent` integer 50–100, default 100 (a row/col is trimmed only while ≥ that % of its pixels match) |
| Keep alpha / PNG output; keep JPEG photos as JPEG | in-model | `format` enum auto\|png\|jpeg, default auto (input JPEG → JPEG q90, everything else → PNG) |
| Clear "whole image is background" failure | in-model | explicit error naming the tolerance, mirrors ImageMagick behavior |
| Before/after report | in-model | result summary states original → trimmed dimensions and per-side pixels removed |

## Out-of-model / not built (listed, not dropped silently)

- **ML background removal preprocessing** (Pico) — needs a segmentation model; gizza is
  pure-Rust + ffmpeg. Background *replacement* already exists separately as
  `blocks/image-bg-replace` (chroma-based).
- **Edge-connected checkbox** (Pico) — not applicable to row/column bounding-box
  trimming: interior background-colored pixels can never remove a content row, because
  scanning stops at the first non-background row/col from each edge. Their toggle only
  matters for per-pixel background *erasure*, which this tool does not do.
- **Batch / bulk trimming, rename, optimize** (Trimmy, PixelForge) — the platform is
  one-file-per-call across all image blocks.
- **Circular trim style** (ImageTools) — a mask/crop-shape feature, covered separately
  by `blocks/image-round-avatar`.
- **SVG input** (Pico) — vector input is out of scope for the raster `image` crate
  pipeline (no rasterizer dependency is wasm-proven here).
- **Standalone web page with slider/color-picker UI** — platform limitation, not a
  design choice: the page generator's file-input path is ffmpeg-runtime only, and pure
  image-bytes output has no page render mode (same as normalize-image,
  image-round-avatar, image-color-quantize…). Surfaces are chat + CLI. ffmpeg
  `cropdetect` could not deliver the alpha-trim table stake anyway (luma-only).

## Design decisions

- Pure-Rust `image` crate (decode png/jpeg/webp/gif/bmp), so the block runs on ALL
  backends including the chat Service Worker — strictly better than an ffmpeg build.
- Background match predicate: max per-channel distance over RGB plus alpha distance
  from opaque (`max(|dr|,|dg|,|db|, 255-a) <= tolerance`); transparent mode matches
  `a <= tolerance`. Default tolerance 16 absorbs anti-aliasing and JPEG ringing.
- `mode=auto`: if ≥3 of 4 corners are transparent at the tolerance → alpha trim; else
  majority corner color (tie → top-left), mirroring the corner-sampling convention both
  ImageTools and ImageMagick document.
- Providing `color` while `mode=auto` switches to that color (explicitly documented in
  the param description); `mode=color` without `color` is an InvalidArgs error.
- "Nothing to trim" returns the image unchanged (re-encoded) with a summary saying so,
  rather than erroring — matches every competitor's behavior.
- Header-first decode budget (~48 MB input+raster) rejects oversized scans with an
  actionable "re-export at lower resolution" error instead of a wasm OOM trap (per the
  document-skew-detector finding).
