## Crop an image in your browser

Pick an image, set the top-left corner (X and Y) and the width and height of the
rectangle you want to keep, and get the cropped copy instantly. The cropping
runs entirely in your browser with ffmpeg compiled to WebAssembly — your image is
never uploaded to a server.

<details>
<summary>How the rectangle works</summary>

<div>
<p>The origin <code>(0, 0)</code> is the top-left corner of the image.</p>
<ul>
<li><strong>X offset</strong> — how far from the left edge the crop starts, in pixels.</li>
<li><strong>Y offset</strong> — how far from the top edge the crop starts, in pixels.</li>
<li><strong>Width</strong> — how wide the kept rectangle is, in pixels.</li>
<li><strong>Height</strong> — how tall the kept rectangle is, in pixels.</li>
</ul>
</div>

</details>

<details>
<summary>Tips</summary>

<div>
<ul>
<li>Keep the rectangle inside the image's dimensions, or ffmpeg will reject it.</li>
<li>Works offline once the page has loaded.</li>
</ul>
</div>

</details>

## FAQ

<details>
<summary>Which image formats can I crop?</summary>

The common web formats — PNG, JPEG, WebP, BMP, GIF — anything the bundled
ffmpeg build can decode. The cropped copy keeps the same format as the
original: crop a `.jpg` and you download a `.jpg`, crop a `.png` and you get
a `.png`.

</details>

<details>
<summary>What if my crop rectangle sticks out past the edge?</summary>

The crop fails: ffmpeg rejects a rectangle that isn't fully inside the image
rather than silently clamping it. Make sure `X + width` stays within the image
width and `Y + height` within the height. Width and height must both be at
least 1 pixel.

</details>

<details>
<summary>Does cropping reduce image quality?</summary>

The pixels you keep are copied 1:1 — no scaling happens — but the file is
re-encoded on save. For lossless formats like PNG that's a perfect copy; for
JPEG the re-encode introduces one generation of compression, as any JPEG
editor does.

</details>

<details>
<summary>Is my photo uploaded to a server?</summary>

No. The crop runs in your browser with ffmpeg compiled to WebAssembly, so the
image never leaves your device — it even works offline once the page has
loaded.

</details>
