## Tile a watermark across a whole image, in your browser

A watermark tucked in one corner is one crop away from gone. This tool repeats
your text over the **entire** picture — the anti-theft pattern stock agencies
put on their previews — so any crop, screenshot or re-upload still carries your
mark. Pick the image, type the text, and the tiled pattern is drawn with ffmpeg
locally: nothing is uploaded, and the original file never leaves your machine.

Everything about the pattern is adjustable: **Text size**, **Text color**,
**Opacity**, the **Angle** of the whole grid (30° is the classic diagonal, 0
gives straight horizontal rows), how many **Tiles across** and **Tiles down**,
and whether alternate rows are staggered (**Brick**) or aligned (**Grid**).
Tile positions are *relative*, so the same settings look identical on a 400px
avatar and a 6000px photo — no re-tuning per image.

### Worked example

Upload `beach.jpg` (1600 × 1200) and use the defaults: text `SAMPLE`, size
`32`, color `#ffffff`, opacity `0.3`, angle `30`, `4` tiles across, `5` down,
**Brick** layout. The output is `beach.jpg` at the same 1600 × 1200 — forty-odd
white `SAMPLE` tiles running diagonally corner to corner, offset row to row,
with no bare triangles in the corners. Over a solid blue area (RGB 0, 0, 255) a
glyph pixel measures RGB (77, 77, 255): exactly 30% white composited over the
photo, so the picture still reads through the mark.

Drop **Opacity** to `0.15` and the same pixel measures RGB (38, 38, 255) — a
whisper-light proofing mark. Raise **Tiles across/down** to `6 × 8` and the
pattern becomes dense enough that cloning it out costs more than licensing the
photo.

### Choosing settings

- **Opacity 0.15–0.25** — proofing and client previews: visible, not intrusive.
- **Opacity 0.3–0.45** — the deterrent default range for public galleries.
- **Opacity 0.6+** — "do not use" stamps; the mark dominates the image.
- **Angle 30–45°** — diagonal text is the hardest to clone out cleanly.
  Angle `0` gives tidy horizontal rows, better for documents and scans.
- **Brick vs Grid** — Brick offsets alternate rows by half a tile, so the
  pattern looks less mechanical and leaves no clean vertical gutter to crop
  along. Grid keeps every row aligned, which suits document stamps.
- **Text size** — start at roughly 2% of the image width (about `32` for a
  1600px photo, `40` for a 2000px one) and adjust from the preview.
- **Black outline** — turn it on when the photo has both bright sky and dark
  shadow; the outline keeps the mark legible on either.

### Limits and edge cases

- **Text**: up to 120 characters, drawn from a text file rather than pasted
  into the filter, so quotes, colons, commas and `%` are all safe. Use `\n` for
  a second line.
- **Density**: 1–12 tiles across and 1–12 down. Very long text at a high tile
  count will overlap between tiles — shorten the text or lower the count.
- **Size**: text size 6–400 px; opacity 0.02–1.0; angle −90° to +90°.
- **Input**: PNG, JPG, WebP, GIF and BMP up to about 25 MB. Animated GIFs keep
  their animation when the output format is *Keep*; converting to PNG/JPG/WebP
  takes the first frame only.
- **Transparency**: JPG output has no alpha channel, so a transparent PNG
  becomes black where it was see-through — choose PNG or WebP to keep it.
- **Not a security control**: a visible watermark deters casual reuse; it is
  not encryption, and a determined editor can still paint parts of it out. Keep
  an un-watermarked master.

## FAQ

<details>
<summary>Will the watermark survive cropping?</summary>

That is the point of tiling. A corner watermark disappears with one crop; a
tiled pattern repeats across the whole frame, so every crop large enough to be
useful still contains several complete tiles. Raise **Tiles across/down** for
smaller crops.

</details>

<details>
<summary>Can I tile a logo image instead of text?</summary>

Not here — this tool tiles **text**. The page takes a single uploaded image, so
there is nowhere to supply a second logo file. To place a logo once (with
position, scale, opacity and blend mode) use the image-composite tool, or type
your studio name here as the repeating mark.

</details>

<details>
<summary>Is my photo uploaded anywhere?</summary>

No. The page loads an ffmpeg build compiled to WebAssembly and runs the whole
watermarking pass inside the browser tab. The image is read from disk into
memory, processed, and offered back as a download — no server round trip, which
is also why it works offline once the page has loaded.

</details>

<details>
<summary>Why does the watermark look darker than the opacity I asked for?</summary>

It shouldn't — and if you have seen that in other tools, this is the reason.
Naively drawing semi-transparent text onto a transparent layer blends the
glyphs against black first and then again during compositing, so 30% opacity
lands nearer 25% and muddied. Here the text is drawn fully opaque and the whole
layer's alpha is scaled once at the end, so 30% opacity composites to exactly
30%.

</details>

<details>
<summary>What size and density should I use for a 4000px photo?</summary>

Density is relative, so **Tiles across/down** need no change at all — `4 × 5`
covers a 400px thumbnail and a 4000px master the same way. Only **Text size**
is in pixels: scale it with the image, around 2% of the width, so about `80`
for a 4000px-wide photo.

</details>

<details>
<summary>Does the output keep my image's format and quality?</summary>

With **Output format** set to *Keep*, the file stays in its original container
(a JPG in, a JPG out) and is re-encoded once. Choose PNG for a lossless result,
WebP for a smaller lossless-ish file, or JPG for the smallest download. JPG
output is encoded at high quality (`-q:v 2`).

</details>
