## Add film grain to a photo, in your browser

Pick an image and a grain amount — analog film grain is overlaid with ffmpeg,
entirely in your browser. Film grain is the fine speckle of silver-halide
photographic film; a touch of it makes a clean digital photo feel organic and
cinematic, hides banding in smooth gradients, and gives a vintage or 35mm
mood. The friendly **Grain amount** (0–100) maps directly onto ffmpeg's noise
strength — `20` (the default) is a subtle film-like texture, `50`+ is heavy,
pushed-film grain.

Leave **Monochrome grain** checked (the default) for neutral gray grain that
varies only brightness — the classic black-and-white film look that stays
natural over color photos. Uncheck it for **colored** grain, where each RGB
channel is speckled independently, the look of pushed high-ISO color film.

### Worked example

Upload `portrait.jpg` and leave **Grain amount** at `20` with **Monochrome
grain** checked — the result keeps the exact same dimensions and the grain
reads as neutral gray speckle. In numbers: on a plain mid-gray test image
(luma 126), monochrome amount `20` leaves the average brightness unchanged
while spreading individual pixels to roughly luma 118–133, and the color
channels stay equal (still neutral gray) — grain, no color shift. At amount
`60` the same pixels spread much wider (about luma 96–155), a heavy grain. If
you uncheck **Monochrome grain**, the red, green and blue channels drift
apart per pixel, so the speckle picks up color.

### Picking an amount

- **5–15** — barely-there texture; kills banding without looking noisy.
- **20–35** — a natural film grain; the default 20 sits here.
- **40–70** — a strong, obvious analog / pushed-film look for moody shots.
- **80–100** — very heavy grain; a stylised, degraded, lo-fi texture.

### Limits and edge cases

- Input files up to 8 MiB; any image format ffmpeg can decode works (PNG,
  JPEG, WebP, BMP, GIF, …). The output keeps the input's exact dimensions —
  nothing is cropped or resized.
- **Output format** `keep` (default) keeps the input format; `png`, `jpg` or
  `webp` convert. Converting takes a single still frame, so an animated GIF
  keeps only its first frame — use `keep` to stay animated. `jpg` output is
  pinned to a high-quality encode (ffmpeg `-q:v 2`) so the grain isn't smeared
  by compression blocks.
- Amount `0` is valid and returns the image unchanged; values outside 0–100
  are rejected with the expected range named.
- The grain is uniform, per-pixel noise with a fixed seed, so the same image
  and amount always produce the same result. Grain *size* (coarse clumps) and
  a separate luma/chroma balance aren't adjustable — ffmpeg's `noise` filter
  works at single-pixel resolution.
- Transparency is not preserved for monochrome grain: it processes in opaque
  YUV, so a transparent PNG comes back fully opaque.

## FAQ

<details>
<summary>What grain amount should I use?</summary>

Start with the default 20 — it adds a natural film texture that most photos
carry well. Drop to 5–15 if you only want to break up banding in skies or
gradients, and push to 40–70 for a deliberately grainy, pushed-film or vintage
look. 80–100 is very heavy, stylised grain that reads as lo-fi rather than
photographic.

</details>

<details>
<summary>What's the difference between monochrome and colored grain?</summary>

Monochrome grain (the default) varies only the brightness of each pixel, so
the speckle is neutral gray — this is how most real black-and-white and even
color film grain looks, and it stays natural over any photo. Colored grain
speckles the red, green and blue channels independently, so the noise picks up
random color — the look of heavily pushed high-ISO color film. Uncheck
**Monochrome grain** to switch to the colored variant.

</details>

<details>
<summary>Does adding grain crop or resize my image?</summary>

No. The output has exactly the same width and height as the input — grain only
changes pixel color, it never crops or scales. If you want to resize or crop as
well, run the image through the image-resize or image-crop tool.

</details>

<details>
<summary>Can I download the result in a different format?</summary>

Yes — set **Output format** to `png`, `jpg` or `webp` to convert while the
grain is applied; `keep` (the default) preserves the input format. JPG
conversion uses a high-quality setting so compression doesn't smear the grain,
and converting an animated GIF keeps its first frame only.

</details>

<details>
<summary>Is my photo uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file locally in the browser tab — the image never leaves your device.

</details>
