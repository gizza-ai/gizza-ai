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
