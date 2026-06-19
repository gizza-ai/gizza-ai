## Crop an image in your browser

Pick an image, set the top-left corner (X and Y) and the width and height of the
rectangle you want to keep, and get the cropped copy instantly. The cropping
runs entirely in your browser with ffmpeg compiled to WebAssembly — your image is
never uploaded to a server.

### How the rectangle works

- **X offset** — how far from the left edge the crop starts, in pixels.
- **Y offset** — how far from the top edge the crop starts, in pixels.
- **Width** — how wide the kept rectangle is, in pixels.
- **Height** — how tall the kept rectangle is, in pixels.

The origin `(0, 0)` is the top-left corner of the image.

### Tips

- Keep the rectangle inside the image's dimensions, or ffmpeg will reject it.
- Works offline once the page has loaded.
