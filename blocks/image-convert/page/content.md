## Convert an image in your browser

Pick an image, choose a target format (JPEG, PNG, or WebP), and get a converted
copy instantly. The conversion runs entirely in your browser with ffmpeg
compiled to WebAssembly — your image is never uploaded to a server.

### Formats

- **JPEG** — small, lossy; great for photos. The quality knob (1-100) controls
  compression.
- **PNG** — lossless; great for graphics and transparency. Quality is ignored.
- **WebP** — modern, smaller than JPEG/PNG at similar quality.

### Tips

- Leave quality blank to use the default (85). It only affects JPEG and WebP.
- Works offline once the page has loaded.

## FAQ

<details>
<summary>What image formats can I convert to and from?</summary>

The **output** is JPEG, PNG, or WebP — those are the three targets the tool
offers. The **input** can be anything the bundled ffmpeg decoder reads, which
covers all the everyday formats: JPEG, PNG, WebP, GIF, BMP, TIFF and more. If
ffmpeg can't decode the file, the conversion fails with an error rather than
producing a broken image.

</details>

<details>
<summary>How does the quality setting work?</summary>

Quality runs from **1 to 100** (default **85**) and only applies to **JPEG and
WebP** — under the hood it's mapped onto ffmpeg's `-q:v` scale. PNG is a lossless
format, so the quality value is ignored entirely when PNG is the target.

</details>

<details>
<summary>What happens to transparency when I convert a PNG to JPEG?</summary>

It's lost — JPEG has no alpha channel, so transparent regions get flattened into a
solid background. If you need to keep transparency while shrinking the file,
convert to **WebP** instead, which supports alpha and usually compresses better
than PNG.

</details>

<details>
<summary>Will converting a JPEG to PNG make it sharper?</summary>

No. PNG stores the pixels losslessly, but the JPEG compression artifacts are
already baked into those pixels — you'll get a much larger file with exactly the
same visual quality. Converting in that direction only makes sense when a workflow
strictly requires a PNG.

</details>
