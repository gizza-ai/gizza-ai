# image-average-color — competitor analysis (2026-07-24)

Tool function: compute the single mean color of an image (plus a gamma-correct /
linear-light variant) as hex/RGB. Scan done BEFORE implementing. All notes are
paraphrased — no competitor copy, branding, or trademarks reproduced.

## Competitors scanned (top of a WebSearch for "average color of image online")

1. **10015.io — Image Average Color Finder.** Offers three methods: *Simple*
   (accumulate RGB/alpha, divide by pixel count), *Square Root* (average of
   squared/sqrt channels — a crude gamma approximation), and *Dominant* (most-used
   color, not a true average). Output: HEX + RGBA, plus a light/dark
   classification. Drag-drop upload, results below the image, copy buttons. No
   region select, no HSL, no complementary.
2. **onlinepngtools.com — Calculate Average Color.** Averages a *list of pasted
   colors* (not image pixels) across eight algorithms incl. Perceptual, Additive
   Light, Subtractive Pigment, Weighted RGB/LAB/LCH/**Linear RGB**/HSL. Accepts and
   emits ~18 color spaces; supports per-color weighting and alpha. This is a
   color-mixing calculator, not an image tool — a different shape from ours.
3. **onlineminitools.com — Average Image Color Finder.** Computes the *arithmetic
   mean of all pixel colors*; outputs HEX, RGB, and HSL with one-click copy; works
   with transparent PNGs. No gamma/perceptual option, no complementary, no region.

(ginifab, codetap, thetoolapp, webtoolscenter appeared in results too — same
image→average shape as #1/#3; nothing beyond the table-stakes below.)

## Table-stakes → decisions (in-model / out-of-model)

| Feature | Decision |
| --- | --- |
| Simple arithmetic mean (sRGB per-channel) | **in-model** — `simple` block in the response |
| Gamma-correct / linear-light mean (the perceptually correct average) | **in-model** — `gamma_correct` block; core ask of this tool. onlinepngtools' "Linear RGB" and 10015's "Square Root" are approximations of this |
| HEX / RGB / RGBA / HSL output | **in-model** — every mean is returned in all four notations |
| Alpha / transparent-PNG handling | **in-model** — `ignore_transparency` bool (default true); near-transparent pixels excluded from the mean, matching color-palette-extract |
| Light/dark classification | **in-model** — `is_dark` + perceived `brightness` (0–100) from the gamma-correct luminance |
| Complementary color | **in-model** — `complementary_hex` (small win no image competitor ships) |
| Dominant color | **out-of-scope** — that is a different tool (already shipped: `blocks/color-palette-extract`, `blocks/image-color-quantize`) |
| Region / rectangular selection of the area to average | **out-of-model** — needs an interactive canvas UI; this is a chat+CLI report tool with no page (see below) |
| Averaging a *list of pasted colors* across 18 color spaces + per-color weights | **out-of-model / different tool** — onlinepngtools mixes swatches, not image pixels; `blocks/color-format-convert` already covers single-color space conversion |
| LAB/LCH/OKLab perceptual means | **considered, rejected** — linear-light (gamma-correct) is the meaningful perceptual mean for a photo; adding 5 more color-space means is schema bloat for a report tool |

## Surface note

Image-input + text/JSON report → **no standalone page** (the established gizza
"F3 no-page file-input" pattern, same as `image-info`, `image-color-picker`,
`color-palette-extract`). The generator's pure-tool page passes field *strings* to
the web export and has no path to hand uploaded image *bytes* to a pure-Rust
decode; ffmpeg pages produce media, not a computed text value. Surfaces built +
verified here: **chat schema (descriptor/drift-guard) + CLI**.
