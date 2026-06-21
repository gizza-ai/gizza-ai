# image-brightness-contrast — competitor analysis & differentiation

**Tool:** `gizza-ai/image-brightness-contrast` — adjust the brightness and
contrast of an image by signed amounts.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `convert -brightness-contrast BxC` (ImageMagick) | CLI | The reference, but a native install and unusual flag syntax. |
| Online "brightness/contrast" tools | Web | Common, but most **upload your image** and re-encode to lossy JPEG. |
| Photo editors (GIMP/Photoshop/Preview) | App | Manual, heavyweight for a quick tweak. |
| CSS `filter: brightness()/contrast()` | Web | Display-only, not baked into the file. |

## How gizza's tool is better / different

1. **Local — image never uploaded.** Runs in WASM (chat SW + CLI). Privacy win.
2. **Lossless PNG output**, alpha + dimensions preserved.
3. **Signed, predictable amounts.** brightness -255..255 and contrast -100..100,
   with 0/0 a true no-op; contrast pivots around mid-gray then brightness adds —
   the familiar editor order.
4. **Wide input support** (PNG/JPEG/WebP/GIF/BMP), two surfaces, one Rust core.

## Verification

Core unit tests assert direction (positive brightness raises values, negative
darkens; positive contrast pushes a sub-midtone pixel darker), alpha+size
preservation, and the 0/0 no-op. **End-to-end CLI** adjusted the Tux PNG
(brightness 60, contrast 20) → a valid 104×120 PNG with changed pixels.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (image-bytes output; same as flip-image /
  normalize-image).
- Global brightness/contrast only (not curves, levels, or per-channel). For
  auto-leveling use `normalize-image`.

## Possible future enhancements

- Gamma adjustment.
- Per-channel brightness/contrast.
- Saturation / hue controls (a colour-adjust sibling).
