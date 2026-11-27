## Brighten, darken or add contrast to a photo, in your browser

Pick an image and move two sliders. **Brightness** adds the same amount to
every color channel — positive values lift the whole picture, negative values
pull it down. **Contrast** stretches or squeezes tones around mid-gray:
positive values make darks darker and lights lighter, negative values flatten
everything toward gray. Contrast is applied first and brightness second, the
same order most photo editors use, so the two sliders behave the way you
expect when you combine them.

The result is always a lossless PNG with the same width, height and
transparency as your original. The image is decoded, adjusted and re-encoded
by WebAssembly on your own device — nothing is uploaded.

### Worked example

A mid-gray pixel (RGB 100, 100, 100) with **Brightness** `50` comes out at
RGB 150, 150, 150. The same pixel with **Contrast** `60` and brightness `0`
gets darker, because it sits below the mid-point (128) that contrast pivots
around; a light pixel at RGB 200 would get lighter instead. Leaving both
sliders at `0` returns the image unchanged apart from the PNG re-encode.

### Choosing values

- **Underexposed photo** — brightness `+30` to `+60`, then a little contrast
  (`+10` to `+20`) so it doesn't look washed out.
- **Flat, hazy shot** — contrast `+20` to `+40` on its own.
- **Overexposed photo** — brightness `-30` to `-60`; very bright skies that
  are pure white can't be recovered, only dimmed.
- **Soft, faded "film" look** — contrast `-25` to `-40` with a small brightness
  lift.

### Limits and edge cases

- Brightness is clamped to −255…255 and contrast to −100…100.
- Transparency is preserved: only the color channels change, not alpha.
- Output is PNG regardless of the input format, so a JPEG may come back
  larger. Convert it afterwards if you need a small file.
- Very large images take longer and need more memory, since every pixel is
  processed on your device.

## FAQ

<details>
<summary>Is my image uploaded anywhere?</summary>
<p>No. The image is read, adjusted and saved in your browser with WebAssembly. Nothing leaves your device.</p>
</details>

<details>
<summary>What's the difference between brightness and contrast?</summary>
<p>Brightness shifts every pixel lighter or darker by the same amount. Contrast changes the distance between dark and light pixels around mid-gray, so it makes an image punchier or flatter without moving its overall level as much.</p>
</details>

<details>
<summary>Why is the result a PNG?</summary>
<p>PNG is lossless, so the adjustment doesn't add compression artifacts and transparency survives. If you need a JPEG or WebP, convert the result afterwards.</p>
</details>

<details>
<summary>Can I undo an adjustment?</summary>
<p>Your original file is never changed. Set both sliders back to 0, or press Reset, to start again from the original.</p>
</details>

<details>
<summary>Does it work on transparent images?</summary>
<p>Yes. The alpha channel is kept exactly as it was; only the red, green and blue values are adjusted.</p>
</details>
