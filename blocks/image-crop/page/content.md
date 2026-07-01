## Crop an image in your browser

Pick an image, set the top-left corner (X and Y) and the width and height of the
rectangle you want to keep, and get the cropped copy instantly. The cropping
runs entirely in your browser with ffmpeg compiled to WebAssembly — your image is
never uploaded to a server.

<details>
<summary>How the rectangle works</summary>

The origin <code>(0, 0)</code> is the top-left corner of the image.

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
