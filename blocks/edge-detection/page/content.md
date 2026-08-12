## About this tool

Edge detection reduces a photo to the boundaries where brightness changes sharply — the
outlines an object-recognition pipeline, a CNC/laser cutter or a vector tracer works from.
Upload an image, pick a method, and the edge map is computed locally by a WebAssembly build
of ffmpeg. Your file never leaves the browser and nothing is uploaded to a server.

### The three methods

| Method | What it does | Best for |
| ------ | ------------ | -------- |
| `canny` | Gaussian smoothing → Sobel gradients → non-maximum suppression (thinning) → hysteresis between the two thresholds. White 1-pixel edges on black. | Clean, connected outlines; tracing; anything you will threshold later |
| `sobel` | Raw gradient magnitude only. Grey edges whose brightness tracks contrast strength — no thinning, no thresholds. | Soft or blurry photos, texture/relief maps, engraving depth |
| `colormix` | Canny detection, but the edges are painted over the original picture instead of replacing it. | An inked/cartoon look that keeps the photo readable |

### Worked example

Take a 640×480 photo of a building, `facade.jpg`, and run the **Clean outline** preset —
`method = canny`, `low = 0.2`, `high = 0.5`, `blur = 1`.

The result is `facade.png`: a 640×480 black image with thin white lines along the window
frames, roofline and door edges, and the brick texture gone (the 1-pixel blur removed it
before detection, and the raised thresholds dropped what was left). Tick **Invert** and the
same run returns black lines on a white background — a printable coloring-page version.
Lower the thresholds to `0.03` / `0.1` instead and the brickwork comes back as a dense mesh
of edges.

### Choosing thresholds

Both thresholds are fractions of full brightness (0–1), not 0–255 values, so the same
numbers work on any image. Hysteresis means: a gradient stronger than **high** always starts
an edge, and the edge is then followed through neighbouring pixels as long as they stay
above **low**. That is why one threshold gives broken dashes and two give continuous lines.

- Start with the defaults (0.078 / 0.196 — the classic 20/255 and 50/255).
- Too much noise? Raise both, or add 1–2 px of blur first.
- Missing faint edges? Lower **low** to about a third of **high**.
- A high:low ratio between 2:1 and 3:1 is the usual recommendation.

### Limits and edge cases

- Maximum input size is **8 MB**; larger files are rejected rather than silently downscaled.
- Input can be PNG, JPEG, WebP, GIF, BMP or TIFF — anything the browser build of ffmpeg
  decodes. Output is PNG (default), JPEG or WebP.
- **Animated** GIF/WebP inputs are reduced to their first frame; this tool returns a still
  image, not an animation.
- `low` must not exceed `high`; the tool reports that instead of producing an empty frame.
- `low` and `high` are ignored when `method = sobel` — that operator has no thresholds.
- Transparency is discarded: `canny` and `sobel` convert to grayscale first, so a PNG with
  an alpha channel is detected against its composited pixels.
- Very large blur values (towards the 10 px cap) will erase all detail and return an almost
  empty edge map — that is the filter working, not a failure.

## FAQ

<details>
<summary>What is the difference between Canny and Sobel edge detection?</summary>

Sobel computes the brightness gradient at every pixel and stops there, so its output is a
soft grey magnitude image where thick, fuzzy bands mark strong contrast. Canny starts with
the same Sobel gradients but adds three steps: smoothing to suppress noise, non-maximum
suppression that thins each ridge down to a single pixel, and hysteresis thresholding that
keeps only edges connected to a strong one. The result is a crisp binary line drawing.
Use Canny when you want outlines to trace, Sobel when you want to see how strong the
contrast is.

</details>

<details>
<summary>Why is my edge map almost empty (or almost solid white)?</summary>

Both symptoms are threshold problems. An empty map means `high` is above nearly every
gradient in the picture — lower `high`, and lower `low` to roughly a third of it. A
solid-white map means the thresholds are so low that sensor noise, film grain and JPEG
blocking all register as edges — raise them, and set `blur` to 1–2 pixels so the noise is
smoothed away before detection. Low-contrast or very dark photos usually need both a lower
threshold pair and a brightness/contrast pass beforehand.

</details>

<details>
<summary>Can I get black lines on a white background for printing?</summary>

Yes — tick **Invert**. The detector always produces white edges on black internally; invert
flips the result, which is the form you want for printing, coloring pages, laser engraving
and most vector-tracing tools. The **Coloring page** preset combines invert with slightly
raised thresholds and a blur pass so only the major outlines survive.

</details>

<details>
<summary>Does this upload my image anywhere?</summary>

No. The page loads a WebAssembly build of ffmpeg and runs the whole filter chain inside the
browser tab, so the picture and the edge map both stay on your machine. The first run
downloads the ffmpeg engine (a few MB, cached afterwards); after that the tool works
offline. The same detection is available from the command line, where files are read
locally too.

</details>

<details>
<summary>What image formats and sizes are supported?</summary>

Input: PNG, JPEG, WebP, GIF, BMP and TIFF, up to 8 MB. Output: PNG (the default, and the
right choice for high-contrast line art), JPEG or WebP. JPEG compresses thin white lines
badly — you will see grey ringing along every edge — so prefer PNG unless file size matters
more than fidelity. Animated inputs contribute only their first frame.

</details>

<details>
<summary>Why is edge detection useful?</summary>

It is the first stage of most classical computer-vision pipelines: shape and contour
detection, document and card boundary finding, OCR pre-processing, and measuring objects in
a scene. Outside vision, edge maps feed CNC routers, laser engravers and vinyl cutters,
serve as the starting point for vector tracing, and are used in image compression research
and in art/illustration workflows where a photo needs to become a line drawing.

</details>
