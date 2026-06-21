# image-color-quantize — competitor analysis & differentiation

**Tool:** `gizza-ai/image-color-quantize` — reduce an image to N colors with an
optimal derived palette.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `convert -colors N` / `pngquant` | CLI | The references, but native installs; pngquant is PNG-in/PNG-out only. |
| Online "reduce colors / posterize" tools | Web | Common, but most **upload your image** and re-encode lossy. |
| Photoshop "Indexed Color" / GIMP | App | Manual, heavyweight. |
| `image` crate's posterize (bit-mask) | Library | Crude per-channel rounding, not a perceptual palette. |

## How gizza's tool is better / different

1. **Local — image never uploaded.** Runs in WASM (chat SW + CLI). Privacy win.
2. **Real, optimal palette (NeuQuant).** The palette is *learned from the image*,
   not a fixed/posterized grid — far better quality than bit-masking, and similar
   in spirit to pngquant.
3. **Any N from 2 to 256.** A hard 2-color reduction for a stark look, or 64/128
   for subtle file-size savings.
4. **Lossless PNG output**, alpha preserved, dimensions kept.
5. **Wide input** (PNG/JPEG/WebP/GIF/BMP), two surfaces, one Rust core.

## Verification

Core unit tests confirm the output has **≤ N distinct colors** and fewer than the
original (a 256-color gradient → ≤8), dimensions are preserved, a solid image
stays a single color, and bad input errors. **End-to-end CLI** quantized the Tux
PNG to 8 colors → a valid 104×120 PNG, smaller than the original (5.1 KB vs
7.7 KB) as expected.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (image-bytes output; same pattern as
  normalize-image / flip-image).
- No dithering (flat quantization). For animated-GIF size reduction see
  `gif-optimize`; for auto-leveling see `normalize-image`.

## Possible future enhancements

- Optional Floyd–Steinberg dithering.
- Output an indexed-PNG (smaller) rather than truecolor PNG.
- Report the chosen palette as hex swatches.
