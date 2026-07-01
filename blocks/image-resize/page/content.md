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

## FAQ

<details>
<summary>Do I need to fill in both width and height?</summary>

No — give just one and the other dimension is computed automatically to keep the
aspect ratio. At least one of the two is required, and both must be positive.
The only mode that insists on both is **cover**, because it has to know the full
box to crop into.

</details>

<details>
<summary>When should I use contain, cover, or stretch?</summary>

**contain** (the default) fits the whole image inside your box without
distortion, so the result can be smaller in one dimension. **cover** fills the
box exactly and crops whatever overflows — ideal for thumbnails and hero images.
**stretch** forces the exact width × height and will distort the picture if the
aspect ratio differs.

</details>

<details>
<summary>Does resizing convert my image to another format?</summary>

No — the output keeps the input's format: a `.jpg` in gives a `.jpg` out, a
`.png` stays `.png`. The file is re-encoded at the new size by ffmpeg, so the
resized copy is a fresh encode rather than a metadata-only change.

</details>

<details>
<summary>Is my photo uploaded anywhere while resizing?</summary>

No. The whole pipeline is ffmpeg compiled to WebAssembly, running inside your
browser tab. The image never leaves your device, and once the page has loaded
the tool keeps working without a network connection.

</details>
