# gif-from-images — competitor analysis & differentiation

**Tool:** `gizza-ai/gif-from-images` — combine a set of images into a single
animated GIF.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `ffmpeg` / `convert -delay … out.gif` (ImageMagick) | CLI | Powerful but a heavyweight native install; frame-size/letterbox handling and palette flags are fiddly. |
| Online "images to GIF / GIF maker" sites | Web | Ubiquitous but **upload your images to a server**, are ad/watermark-heavy, and often cap frame count or resolution on the free tier. |
| ezgif.com etc. | Web | Good features but server-side; privacy + upload cost. |
| Photo apps | App | Manual, heavyweight for a quick animation. |

## How gizza's tool is better / different

1. **Pure-Rust, no ffmpeg — runs everywhere.** GIF encoding is done with the
   `image` crate, so it works in the chat Service Worker and the CLI (all WASM) —
   the picker tagged this "ffmpeg", but it needs none, which means it runs on
   *more* surfaces than an ffmpeg tool could.
2. **Local — images never uploaded.** The decisive privacy advantage over the
   web GIF makers (which is nearly all of them).
3. **Mismatched sizes handled.** Frames are scaled to fit a common canvas (the
   max width/height) preserving aspect ratio and padded with a configurable
   background color — no manual pre-resizing.
4. **Simple, ordered control.** One ordered `images` list + a `delay_ms` per
   frame; loops forever. No flag soup.
5. **Wide input formats.** PNG/JPEG/WebP/GIF/BMP in → GIF out.

## Verification

Core tests build a 3-frame GIF from differently-sized inputs and decode it back
to confirm 3 frames at the common 20×30 canvas, plus the `GIF89a` magic. CLI run
on two fetched PNGs produced a valid 10 KB `GIF89a` with 2 graphic-control
blocks (2 frames) at 300 ms/frame.

## Surfaces & honest scope

- **Chat + CLI only — no web page.** Multi-image (array) input plus image-bytes
  output fits neither the field-input page nor the single-file ffmpeg page; same
  pattern as `image-collage` / `images-to-pdf`.
- GIF is limited to 256 colors per frame (palette quantization by the encoder);
  for photographic animation an MP4 (see `gif-to-mp4`) is smaller/cleaner, but
  GIF maximizes compatibility.

## Possible future enhancements

- Per-frame delays (a parallel `delays` array).
- Finite loop count option.
- Optional fixed output width/height.
