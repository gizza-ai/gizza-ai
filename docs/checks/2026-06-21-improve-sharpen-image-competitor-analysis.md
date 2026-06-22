# sharpen-image — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/sharpen-image` — sharpen an image with an adjustable unsharp
mask. Pure-Rust (`image`). Image input → image (PNG) output, so chat + CLI, no
page (image-bytes output has no page mode — like `normalize-image` /
`image-pixelate-censor`).

## What competitors do

- **Online "sharpen image" sites** — upload, get a sharpened image. Strength:
  easy. **Weakness: the photo is uploaded** to a server; free tiers cap size/day
  and may recompress/watermark.
- **Photoshop / GIMP "Unsharp Mask"** — the reference, with amount/radius/threshold,
  but desktop apps and manual clicking; not scriptable in one call.
- **ImageMagick `-unsharp`** — local + scriptable and excellent, but requires
  installing ImageMagick and learning its `radiusxsigma+amount+threshold` syntax.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`image`) compiled to wasm:
   runs in the chat Service Worker and headless in the CLI. The image never leaves
   the device.
2. **Real unsharp mask, two intuitive knobs.** `amount` (Gaussian sigma — how
   strong) and `threshold` (skip pixels whose local contrast is below it, so flat
   areas / noise aren't amplified) — the controls that actually matter, with sane
   defaults (amount 2.0, threshold 0).
3. **Format-tolerant in, predictable out.** Accepts PNG/JPEG/WebP/GIF/BMP and
   returns a lossless **PNG**, so repeated edits don't accumulate JPEG artifacts.
4. **Chainable + agent-friendly.** Takes the image by `url` or `ref` and returns a
   downloadable PNG envelope (itself a `ref`), so it composes with the other image
   tools; identical from chat and CLI.

## Honest scope

- **Unsharp-mask sharpening** (the standard high-quality method); not deconvolution
  / "AI" super-resolution.
- **PNG output** (lossless); it does not preserve the input's original format or
  metadata.
- **No page** — image input + image-bytes output don't fit the page's text/field
  model (consistent with the other image-editing tools).

## Tests

3 core unit tests on **images assembled in-test**: the output is a valid **PNG of
the same dimensions** (magic bytes + decode check); sharpening a mid-tone
checkerboard **changes pixels** (unsharp overshoot, verified pixel-by-pixel against
the original — using mid-tones so the overshoot isn't clamped as it would be on a
saturated edge); and errors on a non-image and on a non-positive `amount`. Plus
the block drift-guard schema test. **CLI verified** end-to-end on a real photo
(`kernel.org` Tux PNG → a larger sharpened PNG). `wafer build` instantiates the
chat block.
