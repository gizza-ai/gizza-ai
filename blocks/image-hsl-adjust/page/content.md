## Change the colors of a photo with hue, saturation and lightness

Pick an image and move three sliders. Every pixel is converted from RGB to
**HSL** — hue, saturation, lightness — adjusted, and converted back:

- **Hue** rotates every color around the color wheel by the number of degrees
  you choose. `120` turns reds green, greens blue and blues red; `180` swaps
  every color for its complement.
- **Saturation** multiplies how colorful each pixel is. `0` gives grayscale,
  `1` leaves it alone, `1.5` makes colors 50% more intense.
- **Lightness** multiplies how light each pixel is. `0` is black, `1` is
  unchanged, values below `1` darken and above `1` brighten toward white.

The result is a lossless PNG with the same width, height and transparency as
your original. The image is decoded, adjusted and re-encoded by WebAssembly on
your own device — nothing is uploaded.

### Worked example

Take a pure red pixel, RGB 255, 0, 0 (hue 0°, full saturation, 50% lightness):

| Setting | Result |
| --- | --- |
| Hue `180` | RGB 0, 255, 255 — cyan, red's complement |
| Hue `120` | RGB 0, 255, 0 — green |
| Saturation `0` | RGB 128, 128, 128 — mid-gray |
| Saturation `0.5` | RGB 191, 64, 64 — a dusty rose |
| Lightness `0.5` | RGB 128, 0, 0 — dark red |

Saturation `2` on the same red changes nothing: it is already fully
saturated, and saturation can't go above 100%.

### Tips

- **Recolor an object** — try hue shifts in steps of 30° to see the whole
  wheel; skin tones and neutrals move too, so this works best on graphics,
  icons and colorful products.
- **Black and white** — saturation `0` is a quick grayscale: each pixel
  becomes the gray with the same HSL lightness.
- **Muted, filmic look** — saturation `0.6`–`0.8` with lightness `0.9`.
- **Pop for social posts** — saturation `1.2`–`1.4`; above `2` colors start
  to clip and look posterized.

### Limits and edge cases

- Hue is clamped to −360…360 and both scales to 0…4. A full turn (`360` or
  `−360`) gives back the original colors.
- Gray, white and black pixels have no hue, so hue and saturation leave them
  unchanged; only lightness affects them.
- Lightness above `1` pushes everything toward white — a lightness of `2` on
  a mid-tone already reaches pure white.
- Transparency is preserved: only the color channels change, not alpha.
- Output is PNG regardless of the input format, so a JPEG may come back
  larger. Convert it afterwards if you need a small file.

## FAQ

<details>
<summary>Is my image uploaded anywhere?</summary>
<p>No. The image is read, adjusted and saved in your browser with WebAssembly. Nothing leaves your device.</p>
</details>

<details>
<summary>What is HSL?</summary>
<p>HSL describes a color as hue (its position on the color wheel, 0–360°), saturation (how vivid it is) and lightness (how close to black or white it is). Editing in HSL lets you change one of those without touching the others, which is hard to do with raw red, green and blue values.</p>
</details>

<details>
<summary>How do I make an image grayscale?</summary>
<p>Set saturation to 0 and leave hue and lightness alone. Every pixel keeps its lightness but loses its color.</p>
</details>

<details>
<summary>Why didn't the hue slider change my photo much?</summary>
<p>Hue only moves colored pixels. Gray, white and black areas have no hue, and nearly gray areas change only slightly. Increase saturation as well if you want the shift to be more visible.</p>
</details>

<details>
<summary>Does it work on transparent images?</summary>
<p>Yes. The alpha channel is kept exactly as it was; only the red, green and blue values are adjusted.</p>
</details>

<details>
<summary>Can I undo an adjustment?</summary>
<p>Your original file is never changed. Set hue back to 0 and both scales back to 1, or press Reset, to start again from the original.</p>
</details>
