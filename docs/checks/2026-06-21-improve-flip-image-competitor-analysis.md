# flip-image — competitor analysis & differentiation

**Tool:** `gizza-ai/flip-image` — flip an image horizontally (mirror) or
vertically.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Online "flip image" tools | Web | Extremely common, but virtually all **upload your image to a server** to process it, are ad-heavy, and re-encode to lossy JPEG. |
| `convert -flop` / `-flip` (ImageMagick) | CLI | The reference, but a heavyweight native install and flag names (`-flop` = horizontal!) people always look up. |
| Photo editors (Photoshop, GIMP, Preview) | App | Overkill for a single flip; manual. |
| `ffmpeg -vf hflip` | CLI | Works for a frame but ffmpeg for a still image is overkill. |

## How gizza's tool is better / different

1. **Runs locally — image never uploaded.** Chat service worker or CLI, all WASM.
   The opposite of the upload-based web flippers (which is most of them).
2. **Lossless PNG output.** Always re-encodes to PNG, preserving the alpha
   channel — no surprise JPEG recompression.
3. **Plain-language direction.** `horizontal` (mirror) / `vertical` — no
   remembering ImageMagick's confusing `-flip` vs `-flop`.
4. **Wide input support.** Decodes PNG, JPEG, WebP, GIF, and BMP via the Rust
   `image` crate.
5. **Two surfaces, one core.** Chat ("mirror this image") and CLI (`gizza tool
   flip-image`).

## Verification

Unit tests pixel-verify the mapping: a 2×1 red|blue image flipped horizontally
becomes blue|red, and a 1×2 flipped vertically swaps the rows; dimensions are
preserved. CLI run on the kernel.org Tux PNG produced a valid 104×120 PNG
(same dimensions, changed content) — confirmed via PNG-header/IHDR inspection.

## Surfaces & honest scope

- **Chat + CLI only — no web page.** The image-bytes output doesn't fit the
  page's field-input/text-output model (the page file-input path only exists for
  the ffmpeg media runtime). Same pattern as `normalize-image` /
  `image-pixelate-censor`.

## Possible future enhancements

- Combined flip-both (180° via H+V) — though `rotate-image` covers rotation.
- Preserve original format on output instead of always PNG (optional).
