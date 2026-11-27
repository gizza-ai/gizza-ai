## Put a solid border around any image, in your browser

Choose an image, set a **thickness** in pixels and pick a **color**. The tool
builds a new canvas that is larger than your picture by the thickness on every
side, fills it with the border color and places your image, untouched, in the
middle. Nothing is stretched or cropped — the frame is added around the
outside.

The result is a lossless PNG. Decoding, framing and re-encoding all happen in
WebAssembly on your own device — nothing is uploaded.

### Worked example

An 800 × 600 photo with the default settings (thickness `20`, color
`#000000`) comes back as an **840 × 640** PNG: 20 px of black is added to the
left and right (800 + 40) and to the top and bottom (600 + 40). Every pixel in
that outer band is pure black, and the original 800 × 600 picture sits
unchanged starting 20 px in from the top-left corner.

A tiny 2 × 2 image with a 3 px `#0000ff` border becomes 8 × 8 — the original
four pixels end up at positions 3–4 in both directions, surrounded by blue.

### Tips

- **Instagram-style white border** — thickness around 3–5 % of the shorter
  side (e.g. `40` for a 1080 px image) in `#ffffff`.
- **Thin outline for screenshots** — `1` to `4` px in a mid gray such as
  `#888888` so the edge stays visible on both light and dark backgrounds.
- **Gallery matte look** — a wide (`60`–`120` px) off-white like `#f4f1ea`.
- **Translucent frame** — use an 8-digit color with alpha, e.g. `#00000080`
  for half-transparent black; the PNG keeps the transparency.
- To get a double frame, run the result through the tool again with a
  different color and thickness.

### Limits and edge cases

- Thickness is clamped to 0…2000 px per side. `0` returns the image at its
  original size (re-encoded as PNG).
- Colors are accepted as `#rgb`, `#rrggbb` or `#rrggbbaa`; anything else is
  rejected with an "invalid color" error.
- The border is the same width on all four sides.
- Transparent areas inside your image stay transparent — only the new outer
  band takes the border color.
- Output is always PNG, so a framed JPEG may be larger than the original.
  Convert it afterwards if you need a smaller file.
- Very large images and very thick borders need more memory, since the whole
  enlarged canvas is built on your device.

## FAQ

<details>
<summary>Is my image uploaded anywhere?</summary>
<p>No. The image is read, framed and saved in your browser with WebAssembly. Nothing leaves your device.</p>
</details>

<details>
<summary>Does adding a border shrink or crop my picture?</summary>
<p>No. The canvas grows instead: the output is wider and taller by twice the thickness, and every original pixel is kept exactly where it was relative to the others.</p>
</details>

<details>
<summary>Can I make the border semi-transparent?</summary>
<p>Yes. Type an 8-digit hex color such as <code>#ffffff80</code> — the last two digits are the alpha, from 00 (invisible) to ff (solid). The PNG result keeps that transparency.</p>
</details>

<details>
<summary>Can I use different thicknesses on each side?</summary>
<p>Not in one pass — the frame is uniform on all four sides. For an uneven look, add a border and then crop the side you want thinner.</p>
</details>

<details>
<summary>Why is the result a PNG?</summary>
<p>PNG is lossless, so the border edges stay crisp with no compression artifacts and transparency survives. If you need a JPEG or WebP, convert the framed image afterwards.</p>
</details>
