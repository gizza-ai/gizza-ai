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

## FAQ

<details>
<summary>Which image formats can I compress?</summary>

JPG/JPEG, PNG, and WebP — the format is inferred from the file extension and
the output keeps it. Other formats (GIF, BMP, TIFF, …) are rejected; convert
them to one of the three supported formats first.

</details>

<details>
<summary>I lowered the quality but my PNG looks identical — is it working?</summary>

Yes. PNG is a lossless format, so quality can't change the pixels. Instead it
tunes the encoder's effort: a *lower* quality value asks for *harder*
compression (quality 1 maps to the maximum compression level 9), which trades
CPU time for a somewhat smaller file.

</details>

<details>
<summary>Can the "compressed" file come out bigger than the original?</summary>

It can, if the source was already efficiently encoded — re-encoding a
well-optimized JPEG at a high quality setting may add bytes rather than save
them. If that happens, try a lower quality, or convert the image to WebP,
which usually beats JPEG and PNG at the same visual quality.

</details>

<details>
<summary>What does the default quality 80 actually do?</summary>

For JPEG, the 1–100 scale is mapped onto ffmpeg's `-q:v` 31 (worst) to 2
(best), so 80 lands around 8 — a solid size/quality balance. For WebP the
value is passed to the encoder directly, and for PNG it selects the
compression effort.

</details>
