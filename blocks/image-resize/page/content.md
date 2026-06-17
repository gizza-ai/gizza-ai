## Resize an image in your browser

Pick an image, type a width (and optionally a height), and get a resized copy
instantly. The resizing runs entirely in your browser with ffmpeg compiled to
WebAssembly — your image is never uploaded to a server.

### Fit modes

- **contain** (default) — fit inside the box, keep aspect ratio.
- **cover** — fill the box, keep aspect ratio, crop the overflow (needs both width and height).
- **stretch** — force the exact width × height, ignoring aspect ratio.

### Tips

- Give only a width (or only a height) to scale proportionally.
- Works offline once the page has loaded.
