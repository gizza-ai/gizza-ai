## Round the corners of an image, in your browser

Pick an image and a radius. The four corners are cut along a smooth,
anti-aliased arc and the area outside the arc becomes transparent, so the
result drops cleanly onto any background — a slide, a web page, a chat
message or a document. Unlike an avatar cropper, the picture is **not**
squared off: a wide banner stays a wide banner and only its corners change.

**Radius** is measured in pixels by default. Switch **Radius unit** to
`percent` to size it relative to the image's shorter side instead, so the
same setting looks alike on a thumbnail and a full-size photo; `50` percent
rounds the short side completely, turning a square into a circle and a
rectangle into a pill. Leave the radius blank for a gentle default of 12% of
the shorter side. **Corners to round** lets you shape only the top, bottom,
left or right pair, and **Corner fill** replaces the transparency with a
solid color when you need an opaque file.

Everything runs with WebAssembly on your own device — the image is never
uploaded. The result is always a PNG at the original width and height.

### Worked example

Take a 64 × 64 mid-gray image (RGB 128, 128, 128) and set **Radius** to `16`
px with all corners selected. The output is still 64 × 64. Each corner pixel,
such as (0, 0) and (63, 63), now has alpha 0 — fully transparent — while the
center pixel and the middle of each edge stay exactly RGB 128, 128, 128 at
full opacity. Pixels along the curve are partly transparent, which is what
keeps the edge smooth instead of jagged.

Change **Corners to round** to `top` and **Corner fill** to `#ff0000`: the
top-left pixel becomes solid red (255, 0, 0), the bottom corners stay square
and gray, and the whole file is fully opaque.

### Tips

- **Screenshots and UI mockups** — `12` to `24` px matches the corner style of
  most app windows and cards.
- **Profile pictures and logos** — `50` percent on a square image gives a
  perfect circle without cropping anything you didn't ask for.
- **Banners and buttons** — `50` percent on a wide image gives a pill shape.
- **Images stacked on a card** — round just the `top` corners of a header
  image so it meets the card's straight lower edge.
- **Destinations that don't support transparency** (some email clients,
  print) — set **Corner fill** to the background color the image will sit on.

### Limits and edge cases

- The radius is capped at half the shorter side; larger values give the same
  fully rounded result. Percent values are clamped to 0–50.
- A radius below half a pixel leaves the corners square (the image is still
  re-encoded as PNG, with the fill applied if you set one).
- Existing transparency is kept and combined with the rounded mask; with a
  fill color, semi-transparent pixels are blended onto that color.
- Corner fill accepts `transparent`, a hex value (`#rgb`, `#rrggbb`; any alpha
  digits are ignored) or common names such as `white`, `black`, `gray`, `navy`.
- Images up to 8192 px per side are supported. Output is PNG regardless of
  the input format, so a JPEG photo may come back larger.

## FAQ

<details>
<summary>Is my image uploaded anywhere?</summary>
<p>No. The image is decoded, rounded and re-encoded in your browser with WebAssembly. Nothing leaves your device.</p>
</details>

<details>
<summary>Why is the result a PNG and not a JPEG?</summary>
<p>JPEG can't store transparency, so the rounded corners would turn into a solid block. PNG keeps them see-through and is lossless. If you set a corner fill color you can convert the result to JPEG afterwards.</p>
</details>

<details>
<summary>How do I make a circle or pill shape?</summary>
<p>Set the radius unit to percent and the radius to 50. A square image becomes a circle; a rectangle becomes a pill with fully rounded short ends.</p>
</details>

<details>
<summary>Does it crop my image?</summary>
<p>No. The output has exactly the same width and height as the input. Only the pixels outside the corner arcs are made transparent or filled.</p>
</details>

<details>
<summary>Can I round only some corners?</summary>
<p>Yes. Choose top, bottom, left or right under "Corners to round" to shape just that pair and keep the other two square.</p>
</details>

<details>
<summary>Are the edges smooth?</summary>
<p>Yes. Pixels on the curve get partial transparency (anti-aliasing), so the arc looks smooth at any size instead of stair-stepped.</p>
</details>
