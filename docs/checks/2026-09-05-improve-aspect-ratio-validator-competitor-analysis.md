# aspect-ratio-validator — competitor analysis (2026-09-05)

Pre-implementation scan (one `WebSearch` on "aspect ratio calculator checker online 16:9
tolerance validate image dimensions", then the top 3 reachable real tools read directly).
Everything below is **paraphrased**; no competitor copy, branding, or trademarks were reused.

## Competitor profiles

### 1. aspectratiochecker.com — "Aspect Ratio Calculator & Checker"
- **Inputs:** width + height in pixels; drag-and-drop image upload; import-by-URL field.
- **Options:** rounding mode for the resize helper (exact / nearest / down / up / force even
  dimensions); crop preview across several fixed ratios.
- **Outputs:** simplified integer ratio, a name/classification for the ratio, the decimal value,
  plus (for uploads) dimensions, byte size and file type; crop preview reports the share of the
  frame that would be lost.
- **Presets:** one-click 16:9, 4:3, 1:1, 3:2, 9:16, 21:9, 2.35:1, 4:5.
- **Validation:** warns when the ratio is extreme/unusual. No numeric tolerance rule.
- **Positioning:** everything runs client-side; nothing is uploaded.

### 2. calculatorsoup.com — "Aspect Ratio Calculator"
- **Inputs:** width + height with a unit selector (px / in / cm / mm); mode switch between
  solve-for-ratio, solve-for-width, solve-for-height, and resize/compare.
- **Outputs:** the ratio as a decimal (`x:1`) *and* as reduced whole numbers via the greatest
  common factor, the **nearest standard aspect ratio**, the recomputed width/height, and the
  diagonal.
- **Reference data:** compares the computed ratio against a table of ~34 standard display/image
  ratios.
- **Copy angles:** the two ways to express a ratio (decimal vs GCD-reduced), why ratio mismatches
  cause stretching or unwanted cropping, worked 1920×1080 → 16:9 / 1.78:1 example.

### 3. calculateaspectratio.com — "Aspect Ratio Calculator"
- **Inputs:** ratio width + ratio height, and pixel width + pixel height (change one, the partner
  dimension is derived).
- **Presets:** 16:9 widescreen, 9:16 vertical, 21:9 ultrawide, 3:2 DSLR, 4:3 standard, 1:1 square,
  plus free-form custom ratios.
- **Outputs:** the derived matching dimension and a live proportion visualiser.
- **FAQ topics:** what an aspect ratio is, how to use the tool, picking a ratio, why ratio matters,
  ratio vs. pixel count, crop vs. pad when the ratio is wrong, custom ratio support.

## Table stakes → fit decisions

| Table stake (≥1 competitor) | Decision | Where |
| --- | --- | --- |
| GCD-reduced integer ratio (`1920×1080 → 16:9`) | **Build** | `core::analyze` → `ratio` |
| Decimal ratio (`1.778`, and `x:1` form) | **Build** | `ratio_decimal`, `ratio_x_to_1` |
| Nearest standard ratio + friendly name | **Build** — 30-entry standard table incl. cinema (2.39:1, 1.85:1), social (4:5, 9:16), print (3:2), classic (4:3, 5:4) | `nearest_standard*` |
| Orientation (landscape / portrait / square) | **Build** | `orientation` |
| Common-ratio presets / one-click chips | **Build** as `[[example]]` chips (16:9, 9:16, 4:5, 1:1, 21:9, 2.39:1) | `page/meta.toml` |
| "What dimensions would be correct?" fix helper | **Build** — nearest width for the given height, nearest height for the given width, plus the crop/pad deltas | `fix.*` |
| Free-form custom target ratio (`16:9`, `1.85:1`, `16/9`, `1.7778`) | **Build** — one lenient `target` parser | `core::parse_ratio` |
| Client-side / nothing uploaded | **Already true** (wasm, no network) — say so in the page copy | `page/content.md` |
| FAQ covering ratio vs. pixels and crop-vs-pad | **Build** as `<details>` accordions | `page/content.md` |
| Explicit **tolerance-based PASS/FAIL** | **Build — our differentiator.** None of the three enforce a numeric tolerance; they report and leave the judgement to the reader. `tolerance_percent` + `orientation_agnostic` make this a spec gate usable in chat and CI. | `core::analyze` |
| Rounding modes for the resize helper (even dimensions etc.) | **Partial** — the fix helper always returns whole pixels, and `even_dimensions` is offered as a boolean because encoders need even dimensions. Full 4-mode rounding matrix skipped: low value for a validator. | `even_dimensions` |

## Out of model — considered, not built

- **Image upload / drag-and-drop + live crop preview** (aspectratiochecker): this block is a pure
  numeric validator; reading dimensions out of an image file is already
  `blocks/image-info` (it reports `width`, `height` and `aspect_ratio`), so the flow is
  image-info → aspect-ratio-validator rather than a second decoder here.
- **Unit selector (in / cm / mm) + diagonal** (calculatorsoup): physical-display maths, not
  image-spec validation; the ratio is unit-invariant anyway.
- **Live shape visualiser** (calculateaspectratio): a canvas widget, out of scope for the shared
  generic page runtime — the numeric crop/pad deltas carry the same information textually.
- **Platform recommendation tables** ("what ratio for platform X"): editorial data that dates fast
  and would mirror a competitor's table; the preset chips cover the real ratios instead.
