## Rotate a photo by a right angle or any angle, in your browser

Pick an image and enter an angle in degrees. **Positive angles turn the image
clockwise** and negative angles turn it counter-clockwise, so `90` turns a
landscape photo onto its right side, `-90` onto its left, and `180` flips it
upside down.

Right angles (90°, 180°, 270° and their negatives) are **lossless**: every
pixel is moved, none are blended, and width and height simply swap for a
quarter turn. Any other angle — say `2` to straighten a horizon or `45` for a
diamond — is resampled smoothly into a larger canvas that fits the whole
rotated picture, and the corners that become exposed are filled with the
color you choose (transparent by default).

The result is always a PNG. The image is decoded, rotated and re-encoded by
WebAssembly on your own device — nothing is uploaded.

### Worked example

A 4000 × 3000 landscape photo rotated by `90` comes back as a 3000 × 4000
portrait, and the pixel that was in the top-left corner is now in the
top-right corner. The same photo rotated by `3` to fix a slight tilt comes
back as 4152 × 3206: the canvas grows by a little over 150 pixels each way so
no part of the photo is cut off, and the four thin wedges in the corners are
filled with the background color.

### Tips

- **Phone photo shows up sideways** — try `90` first; if it is now upside
  down, use `-90` instead.
- **Straightening a horizon** — small angles between `-5` and `5` are usually
  enough. If the horizon dips toward the right, use a negative value; if it
  rises toward the right, use a positive one.
- **Want no visible corners** — pick a background that matches the photo's
  edge (often `white` for documents), or keep `transparent` and crop the
  result afterwards.
- **Angles wrap around** — `270` and `-90` give the same result, and `360`,
  `-360` and `0` all leave the image unrotated.

### Limits and edge cases

- The angle must be between −360 and 360 degrees.
- Non-right angles enlarge the canvas and fill the exposed corners; they also
  soften the image very slightly because pixels are interpolated.
- The corner fill accepts `transparent`, `white`, `black`, `#rgb`, `#rrggbb`
  or `#rrggbbaa`. It is ignored for 90°, 180° and 270°.
- Output is PNG regardless of the input format, so a JPEG may come back
  larger. Convert it afterwards if you need a smaller file.
- Very large images need more time and memory, since every pixel is processed
  on your device.

## FAQ

<details>
<summary>Is my image uploaded anywhere?</summary>
<p>No. The image is read, rotated and saved in your browser with WebAssembly. Nothing leaves your device.</p>
</details>

<details>
<summary>Which way does a positive angle turn the image?</summary>
<p>Clockwise. Use a negative angle, such as -90, to turn the image counter-clockwise.</p>
</details>

<details>
<summary>Does rotating lose quality?</summary>
<p>Not for 90°, 180° or 270° — those only rearrange pixels and the PNG output is lossless. Other angles blend neighboring pixels, which softens edges a little.</p>
</details>

<details>
<summary>Why did my image get bigger after rotating by 45°?</summary>
<p>A tilted rectangle needs a wider and taller box to fit inside. The canvas is enlarged so nothing is cropped, and the empty corners are filled with the background color.</p>
</details>

<details>
<summary>How do I get transparent corners?</summary>
<p>Leave the corner fill set to transparent (the default). PNG keeps the transparency, so the corners stay see-through when you place the image on another background.</p>
</details>

<details>
<summary>Why is the result a PNG?</summary>
<p>PNG is lossless and supports transparency, so right-angle rotations stay pixel-perfect and transparent corners survive. Convert the result afterwards if you need a JPEG or WebP.</p>
</details>
