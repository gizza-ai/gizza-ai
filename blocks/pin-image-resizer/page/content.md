## Resize an image to a Pinterest pin size in your browser

Pick an image and a pin format — **Standard 1000×1500 (2:3)**, **Square 1000×1000**,
**Tall / infographic 1000×2100**, or **Story / idea 1080×1920 (9:16)** — and get a
correctly-sized pin instantly. Correctly-sized pins fill more of the feed and get
seen more, but hitting the exact ratio by hand is fiddly. This does it in one click,
entirely in your browser with ffmpeg compiled to WebAssembly — your image is never
uploaded to a server.

<details>
<summary>Which fit mode should I use?</summary>

<div>
<ul>
<li><strong>Cover</strong> (default) — scales the image up to fill the whole pin and
crops off whatever overflows. No distortion, no bars; you lose some edges. Best for
photos.</li>
<li><strong>Contain</strong> — scales the image down so all of it fits inside the pin,
then pads the empty space with your chosen background colour. Nothing is cropped.
Best for logos, screenshots, or infographics you can't trim.</li>
<li><strong>Stretch</strong> — forces the exact pin dimensions, distorting the image
if its ratio differs. Use only when the source is already close to the target ratio.</li>
</ul>
</div>

</details>

<details>
<summary>Crop position (Cover only)</summary>

<div>
<p>When <strong>Cover</strong> crops the overflow, <strong>Crop from</strong> decides
which part is kept: <strong>Center</strong> (default), <strong>Top</strong> (keep the
top — good when the subject or a headline sits high in the frame), or
<strong>Bottom</strong>. It has no effect for Contain or Stretch.</p>
</div>

</details>

## Worked example

Drop in a wide 1920×1080 landscape photo and choose **Standard** + **Cover**. The tool
scales it up until it fills the 1000×1500 pin box, then centre-crops the sides that
stick out, so you download a `pin-1000x1500.jpg` at the exact 2:3 ratio Pinterest
recommends — no letterbox bars, no squashing. Switch to **Contain** with a white pad
colour and the same photo is scaled to fit inside 1000×1500 with white bars top and
bottom instead, keeping every pixel of the original.

## FAQ

<details>
<summary>What are Pinterest's recommended pin sizes?</summary>

Pinterest recommends a **2:3 aspect ratio** for standard pins — this tool uses
**1000×1500 px**, the widely cited sweet spot. **Square** pins are **1000×1000**
(1:1). **Tall / infographic** pins can run longer; Pinterest crops anything much
taller than ~1:2.1 in the feed, so this tool caps the long format at **1000×2100**.
**Idea / story** pins are full-screen mobile at **1080×1920** (9:16). Pick the format
and the exact pixels are set for you.

</details>

<details>
<summary>Is my image uploaded anywhere?</summary>

No. The resize and crop run in your browser with ffmpeg compiled to WebAssembly, so
the image never leaves your device — it even works offline once the page has loaded.
There's no account, no watermark, and no file-size upload limit beyond what your
browser can hold in memory (this tool accepts up to 20 MB, matching Pinterest's own
image cap).

</details>

<details>
<summary>Which image formats can I use, and what do I get back?</summary>

The common web formats — PNG, JPEG, WebP, BMP, GIF — anything the bundled ffmpeg
build can decode. The output keeps the same format as the input: resize a `.jpg` and
you download a `.jpg`, resize a `.png` and you get a `.png`. For transparent PNGs,
choose <strong>Contain</strong> and the padding respects your chosen colour.

</details>

<details>
<summary>Will Cover cut off part of my image?</summary>

Yes — that's the point of <strong>Cover</strong>: it fills the whole pin with no bars,
so the parts that don't fit the 2:3 (or chosen) ratio are cropped away. Use
<strong>Crop from: Top</strong> or <strong>Bottom</strong> to control which edges are
kept, or switch to <strong>Contain</strong> to keep the entire image and pad the
difference instead of cropping.

</details>

<details>
<summary>Does resizing reduce image quality?</summary>

The image is re-encoded on save. For lossless formats like PNG that's a clean copy;
for JPEG the re-encode adds one generation of compression, as any image editor does.
Scaling <strong>up</strong> a small source to fill a large pin can look soft — start
from an image at least as large as the target pin (e.g. ≥1000 px wide for a standard
pin) for the sharpest result.

</details>
