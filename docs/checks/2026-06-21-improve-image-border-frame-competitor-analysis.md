# image-border-frame — competitor analysis & differentiation

**Tool:** `gizza-ai/image-border-frame` — add a solid border/frame of a chosen
color and thickness to an image.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `convert in.png -bordercolor red -border 10 out.png` (ImageMagick) | CLI | The reference, but a native install and flag memorization. |
| Online "add border to image" sites | Web | Common, but most **upload your image**, are ad-heavy, and re-encode to lossy JPEG. |
| Photo editors (Photoshop/GIMP/Canva) | App | Overkill and manual for a simple frame. |
| CSS `border` | Web | Only in-browser display, not baked into the file. |

## How gizza's tool is better / different

1. **Local — image never uploaded.** Runs in WASM (chat SW + CLI). Privacy win.
2. **Lossless PNG output**, alpha preserved — no surprise JPEG recompression.
3. **Exact, predictable result.** The output grows by exactly `2 × thickness` in
   each dimension; the border is a solid fill of your color over the original.
4. **Flexible color.** `#rgb`, `#rrggbb`, or `#rrggbbaa` (including a translucent
   frame).
5. **Wide input support** (PNG/JPEG/WebP/GIF/BMP) and two surfaces, one Rust core.

## Verification

Core unit tests check size growth (`+2t`), that corners are the border color and
the interior is the original pixels, the zero-thickness no-op, and color parsing
(incl. `#rgb` / `#rrggbbaa` and rejection of junk). **End-to-end CLI** added a
10px red border to the Tux PNG: 104×120 → **124×140** (exactly +20px per
dimension), valid PNG.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (image-bytes output; same pattern as
  flip-image / normalize-image).
- Uniform border on all four sides; per-side thickness or rounded/inset frames
  are out of scope (kept simple and predictable).

## Possible future enhancements

- Per-side thickness (top/right/bottom/left).
- An *inset* mode (draw the frame over the image instead of growing the canvas).
- A second contrasting inner line (matte effect).
