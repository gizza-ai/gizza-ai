# image-hsl-adjust — competitor analysis & differentiation

**Tool:** `gizza-ai/image-hsl-adjust` — shift hue and scale saturation and
lightness of an image in HSL space.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `convert -modulate L,S,H` (ImageMagick) | CLI | The reference, but a native install and an unintuitive parameter order/units. |
| Photoshop/GIMP Hue-Saturation | App | Manual, heavyweight for a quick shift. |
| Online HSL/hue tools | Web | Common, but most **upload your image** and re-encode lossy. |
| CSS `filter: hue-rotate()/saturate()` | Web | Display-only, not baked into the file. |

## How gizza's tool is better / different

1. **Local — image never uploaded.** Runs in WASM (chat SW + CLI). Privacy win.
2. **True HSL controls.** A **hue shift in degrees** (e.g. 180 → complementary
   colors), plus independent **saturation** and **lightness** scale factors
   (0 = grayscale/black, 1 = unchanged, >1 = more). Grayscale is just
   `saturation=0`.
3. **Correct, tested color math.** Hand-rolled RGB↔HSL with a verified
   round-trip; alpha preserved.
4. **Lossless PNG output**, dimensions kept; wide input
   (PNG/JPEG/WebP/GIF/BMP).
5. Two surfaces, one Rust core, no heavy dependencies.

## Verification

Six core unit tests: RGB↔HSL round-trips within ±1, a 180° hue shift turns red
into cyan, `saturation=0` produces gray (R=G=B), `lightness=0` produces black
with alpha kept, and identity preserves the color. **End-to-end CLI** applied
hue +120°, saturation ×1.5 to the Tux PNG → valid 104×120 PNG with changed
pixels.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (image-bytes output; same as flip-image /
  normalize-image).
- Global HSL transform (not selective by color range / masks). For pure
  contrast/brightness use `image-brightness-contrast`; for auto-levels,
  `normalize-image`.

## Possible future enhancements

- Per-hue-range selective adjustment (like Photoshop's channel dropdown).
- Absolute saturation/lightness set (not just scale).
- HSV variant.
