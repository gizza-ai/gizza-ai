## Compress an image in your browser

Pick an image, set a quality, and get a smaller copy that keeps the **same
format** (JPG stays JPG, PNG stays PNG, WebP stays WebP). The re-encoding runs
entirely in your browser with ffmpeg compiled to WebAssembly — your image is
never uploaded to a server.

### How quality works

- **JPEG / WebP** are lossy: a lower quality throws away more detail for a
  smaller file. Quality **80** (the default) is a good size/quality balance;
  drop to 50–60 for a noticeably smaller file.
- **PNG** is lossless, so quality never changes how the image looks — it only
  tells the encoder how hard to compress. PNG savings are limited; if you need a
  much smaller file, convert it to WebP or JPEG first.

### Tips

- Leave the quality field blank to use the default (80).
- Works offline once the page has loaded.
- To shrink an image to specific pixel dimensions instead, use the
  resize tool.
