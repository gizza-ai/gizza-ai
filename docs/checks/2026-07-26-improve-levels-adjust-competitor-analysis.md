# Competitor analysis — levels-adjust (2026-07-26)

Tool: `levels-adjust` — remaps black point, white point, and midtone gamma from
explicit input/output levels. Built as a **pure numeric** tool: it applies the
photographic "Levels" transfer curve to a list of sample values (0–255 tone
values, luma samples, sensor readings, normalized data), not as a raster image
editor. All notes below are paraphrased; no competitor copy/branding is reused.

## Competitors scanned

1. **Adobe Photoshop — Levels adjustment** (helpx.adobe.com reference). The
   canonical model: three Input sliders (black point, white point, midtone
   gamma) and two Output sliders (output black, output white). Input black/white
   map the chosen input values to the output black/white, redistributing the
   remaining tones between them. The midtone slider is a gamma control shown as
   `1.00` at center; values >1 brighten midtones without moving pure black/white,
   values <1 darken. Per-channel (RGB) curves available. Values are 0–255, always
   clamped.

2. **Vayce — Image Levels Adjustment** (vayce.app). Browser-local. Live
   histogram, black-point / white-point / midtone-gamma controls, gamma range
   ~0.2–5.0 via a master slider plus optional separate R/G/B curves. Output
   level range controls. Reset button.

3. **ImageTools.org — Level** operation. Documents the operation as three
   arguments: black point, white point, gamma. Black/white points set contrast,
   gamma sets brightness. Simple, explicit numeric arguments — closest in spirit
   to a numeric-math tool.

4. **imageonline.io / gamma.imageonline.co — Gamma correction.** Gamma-only
   slider targeting midtones to lift shadow detail without blowing highlights.
   Confirms the `output = input^(1/gamma)` power-law convention on normalized
   [0,1] input.

5. **Reference math — Romz "Levels control shader" (GLSL)** and Photoshop user
   threads. Confirm the three-step transfer: (1) normalize by input black/white,
   (2) apply gamma as a power function on the normalized [0,1] value, (3) remap
   to the output black/white range. Gamma is applied as `norm^(1/gamma)`.

## Table-stakes params (each tagged in-model / out-of-model)

| Capability | Decision |
| --- | --- |
| Input black point | **in-model** → `input_black` (default 0) |
| Input white point | **in-model** → `input_white` (default 255) |
| Midtone gamma | **in-model** → `gamma` (default 1.0) |
| Output black level | **in-model** → `output_black` (default 0) |
| Output white level | **in-model** → `output_white` (default 255) |
| Clamp to range / saturate out-of-range | **in-model** → `clamp` boolean (default true) |
| Explicit numeric input/output arguments | **in-model** → this IS the tool (list of numbers in, remapped numbers out) |
| Worked-example presets (increase contrast, brighten, invert output) | **in-model** → `[[example]]` chips |
| Live histogram of the pixel distribution | **out-of-model** — visual raster feature; we transform a numeric list, not an image, so there is no raster histogram to draw. |
| Per-channel R/G/B curves | **out-of-model** — requires a color raster image; this tool operates on a single numeric channel of samples. A user can run each channel's samples separately. |
| Auto black/white point (auto-levels) from an image histogram | **out-of-model** — needs a full image; a numeric list has no clip-percentile UI. Users pick the min/max they want mapped. |
| Eyedropper set-black/white/gray point | **out-of-model** — interactive raster picking. |

## Design decisions

- **Transfer formula** (matches Photoshop/GLSL reference):
  `norm = (x − input_black) / (input_white − input_black)`; if `clamp`, `norm` is
  clamped to `[0,1]`; `g = signed_pow(norm, 1/gamma)`; `out = output_black + g ·
  (output_white − output_black)`; if `clamp`, `out` is clamped to the
  `[min(output_black,output_white), max(…)]` span.
- **Sign-preserving power** (`sign(n)·|n|^(1/gamma)`) keeps `clamp=false`
  extrapolation finite instead of `NaN` for negative normalized values.
- **Output inversion is a feature**: setting `output_black=255, output_white=0`
  inverts the tones — mirrors Photoshop's swapped output sliders.
- **Errors say what was expected**: `input_white` must differ from `input_black`
  (zero range), `gamma` must be > 0, every token must be a finite number.
- Values default to the classic 8-bit **0–255** domain; any numeric domain works
  because black/white points are explicit.
- Out-of-model raster features (histogram, per-channel curves, auto-levels,
  eyedroppers) are listed here, not built.
