## Replace a green-screen or solid background, in your browser

Upload a photo shot against a **green screen** (or any solid-color backdrop),
name the color to remove, and the subject is cut out and dropped onto a new
background — a **transparent** cut-out, a **solid** color, or a two-color
**gradient**. Everything runs locally with ffmpeg compiled to WebAssembly:
your image is never uploaded, there is no account, and it's free.

This is a **chroma-key** tool: it removes pixels close to one color you choose.
It shines on green/blue screens and clean studio backdrops. It does **not** use
AI to guess the subject, so it can't cleanly cut a person out of a busy street
photo — for that you need a solid or single-color background behind the subject.

### Worked example

Upload a portrait shot on a green screen and click the **Green screen → white**
example, or set the fields by hand:

- **Background color to remove**: `#00ff00` (green-screen green)
- **Similarity**: `30`
- **Edge softness**: `10`
- **New background**: `solid`
- **Background color**: `#ffffff` (white)
- **Output format**: `png`

The green backdrop turns white and the subject stays put — the output keeps the
exact input dimensions. Switch **New background** to `transparent` for a
see-through PNG you can drop onto any design, or to `gradient` with a second
color (say `#87ceeb` → `#ffffff`) for a soft sky backdrop. If your green
lightens at the edges, raise **Similarity** to `40–50`; if the cut-out edge
looks jagged, raise **Edge softness** to `15–25`.

### Tuning the key

- **Similarity** (default 30) — how wide a range of shades near your key color
  counts as background. Start at 30. If patches of the backdrop survive, raise
  it; if the subject starts disappearing, lower it. Uneven lighting on the
  screen usually needs 35–50.
- **Edge softness / blend** (default 10) — feathers the cut-out edge. 0 is a
  hard, aliased cut; 15–25 softens jaggies and hides a thin color fringe. Too
  high makes the whole subject semi-transparent.
- **Background color to remove** — a name (`green`, `lime`, `blue`, `cyan`,
  `white`, `black`, …) or hex (`#00FF00`, `#0F0`). Note CSS `green` is the dark
  `#008000`; use `lime` or `#00ff00` for a real green screen. For a blue screen
  use `#0000ff`.

### New-background options

- **solid** — one flat fill color (`Background color`).
- **gradient** — a smooth blend from `Background color` to `Gradient end
  color`, either `vertical` (top→bottom) or `horizontal` (left→right).
- **transparent** — keep the cut-out's transparency. Requires a `png` or `webp`
  output (jpg has no alpha channel and is rejected with a hint).

### Limits and edge cases

- Input files up to 8 MiB; any format ffmpeg can decode (PNG, JPEG, WebP, BMP,
  …). The output keeps the input's exact width and height — nothing is cropped
  or resized.
- **This is chroma key, not AI segmentation.** It removes one color, so it
  needs a solid/green/blue-screen background behind the subject. Busy or
  multicolor backgrounds can't be removed this way, and any part of the
  *subject* that matches the key color (a green shirt on a green screen) will
  also be cut out — pick a key color your subject doesn't wear.
- **transparent** output must be `png` or `webp`; choosing `jpg` (or `keep`
  over a jpg input) with a transparent background is rejected with guidance.
- **Similarity** and **Edge softness** accept 0–100; values outside that range
  are rejected with the expected range named. Clearing **Similarity** falls
  back to ffmpeg's minimum so you still get a valid result.
- A **second background image** (compositing onto an uploaded photo) isn't
  supported — the page takes a single upload. Use `solid` or `gradient`
  backgrounds here, or layer the transparent PNG over your own image in an
  editor.
- No eyedropper: type or pick the key color rather than clicking it on the
  image.

## FAQ

<details>
<summary>Does this remove the background automatically, like an AI tool?</summary>

No. This is a **chroma-key** remover: it deletes pixels close to one color you
choose, so it needs a solid, green, or blue-screen background behind the
subject. It does not use an AI model to detect the subject, so it can't cut a
person out of a cluttered photo. For clean, controllable results, shoot against
a single-color backdrop and set that color as the one to remove.

</details>

<details>
<summary>What similarity and edge-softness values should I use?</summary>

Start with **Similarity 30** and **Edge softness 10**. If bits of the backdrop
remain (common with uneven studio lighting), raise Similarity to 40–50. If the
subject starts vanishing, lower it. Raise Edge softness to 15–25 to smooth a
jagged or fringed cut-out edge; keep it low for crisp graphics. Very high
values make the whole subject partly transparent, so nudge in small steps.

</details>

<details>
<summary>Can I get a transparent PNG instead of a colored background?</summary>

Yes. Set **New background** to `transparent` and **Output format** to `png` (or
`webp`) — the subject comes back with a see-through background you can layer
onto any design. A `jpg` output can't hold transparency, so it's rejected with
a hint if you combine it with a transparent background.

</details>

<details>
<summary>Can I use a blue screen or a custom backdrop color?</summary>

Yes. Set **Background color to remove** to your backdrop's color — `#0000ff`
for a blue screen, or any name/hex for a custom solid backdrop. A slightly
higher Similarity (35–45) often helps with blue screens because skin tones sit
closer to blue than to green. Avoid a key color your subject is wearing, or it
will be cut out too.

</details>

<details>
<summary>Can I replace the background with my own image?</summary>

Not directly — the page accepts a single upload, so it can composite onto a
solid color or a two-color gradient, but not onto a second uploaded photo.
The workaround is to export a **transparent PNG** here and place it over your
background image in any editor or design tool.

</details>

<details>
<summary>Is my photo uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once, then processes your
file entirely in the browser tab. The image never leaves your device, and
there's no account or sign-in.

</details>
