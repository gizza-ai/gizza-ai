## Perspective-warp a photo, in your browser

Pick an image and set where its four corners should sit — a perspective
transform is applied with ffmpeg, entirely in your browser. Corner fields accept
pixel coordinates, plus ffmpeg's built-in `W` and `H` constants for the right and
bottom edges. Two things you'll do most:

- **Straighten a skewed rectangle** — a photo of a whiteboard, book page, or
  building shot from an angle looks like a keystone (its edges lean in). Switch
  **Mode** to `correct`, then set the four corners to where the *actual*
  rectangle sits in the tilted photo; the tool stretches that region to fill the
  frame, so a leaning document comes out square.
- **Add a deliberate tilt** — switch **Mode** to `distort` and push the image's
  corners to new positions for a 3-D, laid-back, or floating-card look. The
  far right and bottom defaults are `W` and `H`, so identity is `0,0` · `W,0` ·
  `0,H` · `W,H`.

**Interpolation** trades speed for smoothness: `linear` (default) is fast;
`cubic` resamples with smoother diagonal edges, useful for text and fine lines.

### Worked example

For a 640×640 photo of a document shot slightly from the left, leave **Mode** on
`correct` and set the corners to the document's real pixel corners — say
**Top-left** `80, 50`, **Top-right** `560, 50`, **Bottom-left** `15, 610`,
**Bottom-right** `625, 610`. The tool builds the ffmpeg filter
`perspective=x0=80:y0=50:x1=560:y1=50:x2=15:y2=610:x3=625:y3=610:interpolation=linear:sense=source`
and the skewed page is pulled back into a clean rectangle at the same output
size. Want the opposite? Try the **Inset 64px frame** preset in `distort` mode
on a small fixture: the image is pushed inward and remapped by ffmpeg.

### Correct vs distort — which mode?

- **correct** (`sense=source`) — the four corners describe where a rectangle
  *currently* sits in your (skewed) photo, and that region is stretched to fill
  the whole output. Use it to **remove** perspective: deskew scans, flatten
  keystoned slides, square up a photographed sign.
- **distort** (`sense=destination`) — the four corners are where the image's own
  corners get **pushed to**. Use it to **add** perspective: tilt a screenshot,
  fake a 3-D card, make a mockup lean.

An identity setup (`0,0` · `W,0` · `0,H` · `W,H`) is a no-op in either mode and
returns the image unchanged — a safe starting point.

### Limits and edge cases

- Input files up to 8 MiB; any image format ffmpeg can decode works (PNG,
  JPEG, WebP, BMP, GIF, …). The output keeps the input's exact dimensions —
  the frame is never resized, only the content inside it is remapped.
- Corner coordinates accept pixel numbers from `-100000` to `100000`, or exactly
  `W`/`H` for the image width/height edge. Arithmetic expressions such as
  `0.12*W` are deliberately rejected because browser ffmpeg builds do not
  evaluate them consistently for the perspective filter.
- The four corners should form a real quadrilateral. If all numeric corners are
  collinear or collapse to (nearly) the same point — zero area — the run is
  rejected with a hint.
- Areas exposed by a `distort` warp use ffmpeg's own edge handling; there is no
  custom background-fill color option on this filter.
- The output format matches the input. To change format, run the result through
  a dedicated image-convert tool afterward.
- Transparency is not preserved: processing happens in opaque planar RGB, so a
  transparent PNG comes back fully opaque.

## FAQ

<details>
<summary>What's the difference between correct and distort mode?</summary>

In **correct** mode the four corners describe where a rectangle *already sits*
in your (skewed) photo, and that region is stretched to fill the frame — this
*removes* perspective, so a slanted document comes out square. In **distort**
mode the four corners are where the image's own corners get *pushed to*, which
*adds* perspective for a tilt or 3-D effect. Same four inputs, opposite
direction.

</details>

<details>
<summary>How do I straighten a photo taken at an angle (keystone correction)?</summary>

Use **correct** mode and set each corner to where the true rectangle's corner
lands in your tilted photo. For a 640px-wide whiteboard shot from below, its top
edge may look narrower, so you might set **Top-left** to `80, 40` and
**Top-right** to `560, 40` while the bottom corners stay near `0,H` and `W,H`.
The narrower top is stretched wider and the board comes out square.

</details>

<details>
<summary>Why do I enter pixels instead of percentages?</summary>

ffmpeg's perspective filter reliably accepts pixel numbers and its `W`/`H`
edge constants. Some ffmpeg builds accept arithmetic expressions in other
filters, but the browser build used here does not consistently evaluate
`0.12*W`-style expressions for perspective coordinates. Pixel fields are more
honest: they match what actually runs in the CLI and page.

</details>

<details>
<summary>Does warping change my image's size or resolution?</summary>

No. The output has exactly the same width and height as the input. A
perspective warp only remaps where pixels land inside that fixed frame; it
never crops or rescales the canvas. If you also need to crop or resize, run the
result through the matching crop/resize tool.

</details>

<details>
<summary>When should I use cubic interpolation instead of linear?</summary>

`linear` (the default) is fast and fine for most photos. Switch to `cubic` when
the warp is strong or the image has crisp diagonal edges — text, line art,
UI screenshots — where cubic resampling produces smoother, less jagged edges.
It's a little slower but usually worth it for documents.

</details>

<details>
<summary>Is my photo uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file locally in the browser tab — the image never leaves your device.

</details>
