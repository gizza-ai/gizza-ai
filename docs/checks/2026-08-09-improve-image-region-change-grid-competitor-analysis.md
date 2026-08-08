# image-region-change-grid — competitor analysis (2026-08-09)

Scan run **before** implementing, per `.claude/skills/create-tool-loop/SKILL.md` step 4.
All findings are paraphrased from public product/docs pages — **no competitor copy, branding, or
trademark text is reproduced or reused** anywhere in the block.

## What this tool is

Take two *aligned* images (a before and an after of the same scene/screen), cut them into a
`columns × rows` grid, and report **which cells changed and by how much** — a compact
"what changed where" summary as text/JSON rather than a picture. The output is a table of cells
with a changed-pixel percentage and a mean/max delta per cell, plus an ASCII heat-map so the
spatial answer survives being pasted into a chat or a terminal.

## Competitors reviewed (top 3 real tools + 2 reference libraries)

| # | Tool | What it does | Reachable |
|---|------|--------------|-----------|
| 1 | img2go "Compare Image" (online) | Uploads two images, runs an ImageMagick-style metric, emits a diff image + numbers | yes |
| 2 | diff.tools "Image Compare" (online) | Multi-view pixel/perceptual comparison workbench with "regions of difference" detection | yes |
| 3 | Aback Tools / Loopaloo / FreeDiffChecker image-diff family (online) | Browser-local pixel diff with a sensitivity slider and a percent-changed readout | yes |
| R | mapbox/pixelmatch (library) | The de-facto pixel-diff primitive used by visual-regression suites | yes |
| R | rsmbl/Resemble.js (library) | Pixel diff with ignore-colour / ignore-antialiasing / ignore-region modes | yes |

### 1. img2go — Compare Image

- Offers a **choice of comparison metric**: SSIM, PSNR, MAE, RMSE, AE (absolute error = count of
  differing pixels), NCC.
- **Threshold 0–100** to control sensitivity.
- Reports **per-channel error counts** (R/G/B), a total **difference percentage**, and an
  **absolute changed-pixel count**; shows a file-metadata table next to the diff.
- Diff highlight colour is user-selectable from a fixed palette; PDF export of the report.
- Accepts JPG/JPEG/PNG/WebP/GIF/BMP/TIFF.
- Stated limit: results are only meaningful when the two images have **matching dimensions**.
- Free tier is metered (a couple of comparisons per day).

### 2. diff.tools — Image Compare

- View modes: two-up, blink, split (vertical/horizontal/diagonal divider), and a difference render;
  synchronised zoom/pan.
- **Regions of Difference (ROD)**: a *threshold slider* **plus a minimum-region-size filter** so
  scattered noise does not register, and a list of detected change regions to step through.
- Engines: strict pixel mode, ΔE00 colour-difference map, FLIP (perceptual), SSIM map.
- **Mean / max readout** quantifying the difference numerically.
- Handles **mismatched sizes** with a scale-normalising mode (e.g. @2x vs @1x).
- Also diffs EXIF metadata.
- Stated limit: needs a current Chromium/Safari with WebGPU.

### 3. Aback Tools / Loopaloo / FreeDiffChecker family

- Two uploads, 100% in-browser.
- **Adjustable sensitivity threshold** — "how much colour difference counts as a change".
- Reports a **difference ratio**: changed pixels ÷ compared pixels, as a percent (and its inverse,
  a similarity percent).
- Output is a highlight/heat-map image (changed pixels glow) plus a downloadable diff.

### R. pixelmatch (library, for defaults)

- `threshold` is **0–1, default 0.1** — smaller is more sensitive; it is a normalised colour
  distance (YIQ-weighted), not a raw channel delta.
- `includeAA` (default `false`) — anti-aliased pixels are detected and ignored by default.
- **`windowSize: N`** changes the return value to the largest diff-pixel count found in any N×N
  region, explicitly so the result is *robust to scattered noise* — i.e. the same block-wise idea
  this tool generalises into a full labelled grid.

### R. Resemble.js (library, for options)

- Ignore modes: ignore-colour (compare brightness only), ignore-antialiasing, ignore-nothing.
- Ignored **rectangular areas** can be excluded from the comparison.
- Returns `misMatchPercentage` and a bounding box of the changed area.

## Table stakes → where each one landed

Every item below ends in the descriptor **or** in the out-of-model list. Nothing dropped silently.

| Table stake | Seen in | Decision |
|---|---|---|
| Two-image comparison | all | **In model** — `images` source list, exactly 2 (before, after) |
| Sensitivity / threshold for "what counts as changed" | img2go, diff.tools, Aback, pixelmatch | **In model** — `threshold`, 0–100 % colour distance, default **2** (≈ pixelmatch's 0.1 on a 0–1 scale, which is the industry default) |
| Percent of pixels changed | all | **In model** — `changed_percent` overall and per cell |
| Mean / max difference readout | diff.tools, img2go (MAE/RMSE) | **In model** — `mean_delta_percent` + `max_delta_percent`, overall and per cell |
| Absolute changed-pixel count | img2go, Aback | **In model** — `changed_pixels` / `total_pixels`, overall and per cell |
| Minimum-region-size filter to suppress noise | diff.tools (ROD) | **In model** — `min_change`, the per-cell changed-% at which a cell is *flagged*; default **1** |
| Block/region-wise robustness instead of per-pixel noise | pixelmatch `windowSize` | **In model** — this is the tool's core: `columns` × `rows` grid, default **4 × 4** |
| Choice of difference metric | img2go, diff.tools, Resemble | **In model (subset)** — `metric` = `rgb` (Euclidean RGB, default) / `luma` (perceived brightness, i.e. ignore-colour) / `max-channel` (strictest per-channel). SSIM/FLIP/ΔE00 are out-of-model (below) |
| Ignore colour, compare brightness only | Resemble.js | **In model** — `metric = luma` |
| Mismatched dimensions handling | diff.tools (scale-normalise), img2go (states the limit) | **In model** — `size_mismatch` = `resize` (default; scale the second image onto the first's canvas) or `error` |
| Change *map* / heat-map visualisation | all three online tools | **In model, text form** — `map` boolean (default on) renders an ASCII density grid + legend, the pasteable equivalent of a heat-map. A raster heat-map PNG is out-of-model (see below) |
| Wide format support | img2go, diff.tools | **In model** — PNG/JPEG/WebP/GIF/BMP via the `image` crate |
| Ranked list of the biggest changes to step through | diff.tools ROD list | **In model** — `top_cells`, cells sorted by changed-% |
| Alpha/transparency handled sanely | implicit | **In model** — RGBA compared, alpha included in the RGB metric |

## Out of model (listed, not built)

- **SSIM / PSNR / NCC / ΔE00 / FLIP perceptual engines** (img2go, diff.tools). Each is a distinct
  research-grade metric with its own windowing and colour-space machinery; `rgb`/`luma`/`max-channel`
  cover the "how different is this pixel" question the grid needs. Revisit only if a specific
  perceptual metric is asked for by name.
- **Rendered diff/heat-map image output** (all three online tools). This block emits a text/JSON
  report by design — that is the "compact what-changed-where summary" the backlog row asks for.
  Pure-Rust image-bytes output would also mean no page surface, and the visual-overlay niche is
  already covered by `blocks/image-split-overlay`.
- **Interactive views** — split slider, blink, two-up, synchronised zoom/pan, drag-to-reveal
  (diff.tools, texloom). No gizza surface has an interactive image canvas (same class as the
  skiplisted `pixel-art-editor` / `chart-image-digitizer`).
- **Anti-aliasing detection/exclusion** (pixelmatch `includeAA`, Resemble). Pixel-neighbourhood
  AA classification is a per-pixel heuristic aimed at *pixel* diffs; grid aggregation plus
  `min_change` already suppresses AA noise, which is the reason `windowSize` exists in pixelmatch.
- **User-drawn ignore regions** (Resemble). Needs an interactive canvas to select rectangles.
- **EXIF / metadata diff** (diff.tools). Separate concern; `blocks/image-metadata-viewer` covers
  metadata inspection.
- **Diff-highlight colour picker, PDF export** (img2go). Both are properties of a rendered image
  report, which this tool does not produce.

## Surfaces

Two image inputs ⇒ `Param::source_list` + `Vec<SourceFields>`, the proven multi-image pure pattern
(`blocks/image-collage`, `blocks/duplicate-image-finder`, `blocks/images-to-pdf`). The generated
page form is a **single** file upload with no second-file control, so — exactly as for
`duplicate-image-finder` — this block ships **chat + CLI, no page**. Verification is therefore the
CLI exact-output + advertised-values matrix plus `cargo test --workspace`, not Playwright.

## Defaults chosen (and why)

| Param | Default | Rationale |
|---|---|---|
| `columns` / `rows` | 4 / 4 | 16 cells reads at a glance in chat and in a terminal; competitors' region detectors surface a handful of regions, not hundreds |
| `threshold` | 2 (%) | pixelmatch ships 0.1 on a 0–1 normalised scale as its default sensitivity; 2 % of the max colour distance is the same order and tolerates JPEG re-compression |
| `min_change` | 1 (%) | diff.tools' minimum-region-size filter exists to drop specks; 1 % of a cell is the smallest change worth calling out |
| `metric` | `rgb` | Strict pixel mode is every competitor's default engine |
| `size_mismatch` | `resize` | diff.tools scale-normalises rather than refusing; `error` stays available for strict regression use |
| `map` | `true` | Every online competitor leads with a visual change map; the ASCII grid is its pasteable analogue |
