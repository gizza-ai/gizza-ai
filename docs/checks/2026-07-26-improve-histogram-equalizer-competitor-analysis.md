# histogram-equalizer — competitor analysis (2026-07-26)

Scan done BEFORE implementation to fix table-stakes, defaults, and UX for the new
`histogram-equalizer` tool (enhance contrast via global + adaptive/CLAHE histogram
equalization). All notes are paraphrased — no competitor copy/branding reused.

## Competitors skimmed (top real tools)

1. **OpenCV** (`cv2.equalizeHist`, `cv2.createCLAHE`) — the reference implementation everyone
   copies. Global `equalizeHist` operates on a single-channel (grayscale) image. CLAHE via
   `createCLAHE(clipLimit=40.0, tileGridSize=(8,8))`: image split into an 8×8 grid of tiles, each
   tile histogram-equalized, histograms clipped at `clipLimit` and the excess redistributed, then
   bilinear interpolation across tile boundaries to remove seams. For color, the documented recipe
   is: convert to a luma+chroma space (YCrCb / LAB), equalize ONLY the luma channel, convert back —
   equalizing R/G/B independently shifts colour balance.
2. **ImageMagick** — ships BOTH as separate operators: `-equalize` (global HE, per-channel by
   default) and `-clahe {width}x{height}%{bins}+{clip}` (adaptive). Tile size given as width×height
   (px or %), a histogram bin count, and a clip limit. Confirms global vs adaptive are two distinct
   features, not one.
3. **Pixlane — CLAHE Online** (pixlane.media/histogram-equalize) — browser-only, no upload. Offers
   "CLAHE or standard histogram equalization"; markets adaptive contrast for "dark, low-contrast
   photos." Toggle between adaptive (CLAHE) and plain global HE.
4. **Image Tool Hub — Histogram Equalization** — browser-local. Explicitly exposes: Global HE **and**
   CLAHE, a **clip limit** control, a **tile size** control, and colour modes: **luminance**,
   **per-channel**, and **grayscale**. This is the fullest feature set and drove our parameter list.
5. **ImageJ — Enhance Local Contrast (CLAHE)** plugin — parameters "blocksize" (tile size, default
   127 px), "histogram bins" (256), and "max slope" (default 3) — a normalized clip limit expressed
   as a slope multiple of the average histogram height (our `clip_limit` follows this normalized
   convention rather than OpenCV's raw 40).

## Table stakes (paraphrased) → our decision

| Capability | Competitors | Our tool (in-model?) |
|---|---|---|
| Global histogram equalization | OpenCV, ImageMagick, Pixlane, Image Tool Hub | ✅ `method=global` |
| Adaptive CLAHE (tiled, bilinear-interpolated, clip-limited) | all 5 | ✅ `method=adaptive` (default) |
| Clip limit control | OpenCV (40), ImageJ (max-slope 3), ImageMagick, Image Tool Hub | ✅ `clip_limit` (normalized slope, default 2.0, 1–40) |
| Tile grid / block size | OpenCV (8×8), ImageJ (127px), ImageMagick, Image Tool Hub | ✅ `tile_grid` (tiles/axis, default 8, 1–32) |
| Luminance mode (preserve colour) | OpenCV recipe, Image Tool Hub | ✅ `channel_mode=luminance` (default) — equalize Y in YCbCr |
| Per-channel mode | ImageMagick default, Image Tool Hub | ✅ `channel_mode=per_channel` |
| Grayscale output | OpenCV global, Image Tool Hub | ✅ `channel_mode=grayscale` |
| Browser-local, no upload | Pixlane, Image Tool Hub | ✅ runs as wasm chat block + CLI, no server |

## Defaults chosen

- `method = adaptive` (CLAHE) — the headline "fixes dark/low-contrast photos" behaviour; global HE
  available for the classic full-image transform.
- `channel_mode = luminance` — the safe default that enhances contrast without shifting hue
  (OpenCV's documented colour recipe).
- `tile_grid = 8` — OpenCV's `tileGridSize=(8,8)` default.
- `clip_limit = 2.0` — normalized slope (ImageJ-style; 1 = strong limiting/near-uniform, higher =
  more local contrast/noise). Range 1–40 covers OpenCV's scale.

## UX / control patterns

- Fixed choices (`method`, `channel_mode`) → `Param::enumv` (LLM-facing enum; would render a
  `<select>` on a page). Numeric `tile_grid`/`clip_limit` bounded with `.min()/.max()`.
- This is an image-bytes-output tool → **chat + CLI only, no standalone page** (same shape as
  `normalize-image` / `image-false-color`; PNG bytes have no page render mode in this repo).

## In-model vs out-of-model

- **In-model, built:** everything in the table above (pure-Rust `image` crate; deterministic).
- **Out-of-model (considered, not built):** live in-browser preview slider UI (needs the private
  site page layer — this repo ships chat+CLI); batch/folder processing (needs a server);
  region-of-interest masks (needs interactive canvas). None fit gizza's browser-local wasm model
  for an image-bytes tool.

Sources: [OpenCV CLAHE (PyImageSearch)](https://pyimagesearch.com/2021/02/01/opencv-histogram-equalization-and-adaptive-histogram-equalization-clahe/), [ImageMagick CLAHE](https://imagemagick.org/clahe/), [Pixlane](https://pixlane.media/histogram-equalize/), [Image Tool Hub](https://www.imagetoolhub.com/tools/histogram-equalization), [ImageJ CLAHE](https://imagej.net/plugins/clahe), [Adaptive histogram equalization (Wikipedia)](https://en.wikipedia.org/wiki/Adaptive_histogram_equalization).
