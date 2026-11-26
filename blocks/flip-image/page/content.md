## Mirror or flip a picture, in your browser

Flipping an image makes a mirror copy of it. A **horizontal** flip swaps left
and right, exactly like holding the picture up to a mirror: whatever faced
left now faces right. A **vertical** flip swaps top and bottom, turning the
picture upside down *and* mirrored — which is different from rotating it by
180°, where left and right also trade places.

Pick an image, choose a direction and the flipped copy appears straight away.
The image is decoded, flipped and re-encoded with WebAssembly on your own
device — nothing is uploaded. The result is a lossless PNG with the same
width, height and transparency as the original.

### Worked example

Take a 64 × 64 image split into four colored squares: red top-left, green
top-right, blue bottom-left, white bottom-right. A **horizontal** flip moves
the pixel at column 63 of the top row to column 0, so the top-left corner is
now green and the top-right corner is red; blue and white trade places along
the bottom. A **vertical** flip instead moves row 63 to row 0, so the
top-left corner becomes blue and the bottom-left becomes red. No pixel
changes color — only its position moves.

### Tips

- **Fix a mirrored selfie** — many front cameras save photos the way the
  preview looked, so text on a shirt or sign reads backwards. A horizontal
  flip puts it the right way round.
- **Reversed text or a logo** — the same fix applies to a scan or screenshot
  of anything written backwards, such as a photo taken through a window.
- **Make a reflection** — flip a copy vertically and place it under the
  original for a water or glass reflection effect.
- **Iron-on transfers and t-shirt prints** — these are usually printed
  mirrored; flip horizontally before printing so the design comes out right.
- Flipping twice in the same direction gives you back the original image.

### Limits and edge cases

- Only horizontal and vertical flips are offered; to turn an image by 90° or
  180°, use a rotate tool instead.
- Transparency is kept exactly as it was.
- The output is always PNG regardless of the input format, so a JPEG may
  come back larger. Convert it afterwards if you need a smaller file.
- Animated GIFs are read as their first frame only.
- Very large images take longer and use more memory, since the work happens
  on your device.

## FAQ

<details>
<summary>Is my image uploaded anywhere?</summary>
<p>No. The image is read, flipped and saved in your browser with WebAssembly. Nothing leaves your device.</p>
</details>

<details>
<summary>What's the difference between horizontal and vertical?</summary>
<p>Horizontal swaps left and right, like a mirror. Vertical swaps top and bottom, turning the picture upside down while also mirroring it top-to-bottom.</p>
</details>

<details>
<summary>Is a vertical flip the same as rotating 180°?</summary>
<p>No. Rotating by 180° is a horizontal flip plus a vertical flip together. A vertical flip on its own leaves left and right where they were, so text reads upside down and backwards differently than it would after a rotation.</p>
</details>

<details>
<summary>Does flipping reduce image quality?</summary>
<p>No. Pixels are only moved, never recolored, and the result is saved as lossless PNG, so the picture is identical apart from its orientation.</p>
</details>

<details>
<summary>Why is my selfie mirrored in the first place?</summary>
<p>Front cameras often show a mirrored preview so moving feels natural, and some phones save the photo that way too. A horizontal flip turns it into how other people actually see you.</p>
</details>
