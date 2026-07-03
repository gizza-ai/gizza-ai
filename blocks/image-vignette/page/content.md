## Add a vignette to a photo, in your browser

Pick an image and a strength — a vignette is applied with ffmpeg, entirely in
your browser. A vignette darkens the image gradually toward the edges and
corners, pulling the viewer's eye to the middle of the frame; it's the classic
finishing touch for portraits, product shots, and moody landscapes. Switch
**Mode** to `lighten` to brighten the edges instead for a faded, hazy border,
or pick a **Vignette color** (a name like `sepia` or hex like `#1A2B3C`) to
fade the edges toward any tint — white for an airy high-key border, sepia for
a vintage look, or a brand color for thumbnails. Under the hood the friendly
**Strength** value (0–100) is mapped onto the ffmpeg `vignette` filter's
angle — the default `40` lands exactly on the filter's classic default, and
you never have to think in radians.

### Worked example

Upload `portrait.jpg` and leave **Strength** at `40` — the result keeps the
same dimensions, the face in the middle stays at full brightness, and the
corners fall off softly. In numbers: on a plain white test image, strength
`40` leaves the center at RGB 255 while the far corners drop to roughly
RGB 110; at strength `80` the corners are nearly black (about RGB 3), and at
`100` they reach pure black. With **Vignette color** `#B08050` and strength
`100`, the corners land exactly on RGB (176,128,80) while the center stays
untouched. If your subject sits on the left third, set **Center X** to `25` —
the bright spot follows the subject and the right side darkens more.

### Picking a strength

- **15–30** — barely-there edge falloff; adds depth without being noticed.
- **35–55** — the classic photographic vignette; the default 40 sits here.
- **60–85** — dramatic, moody edges for portraits and posters.
- **90–100** — corners go fully to the vignette color (black by default,
  white in lighten mode); a spotlight/tunnel effect rather than a subtle
  finish.

### Picking a color

- **black** (default) — the classic lens vignette; uses the plain filter.
- **white / ivory** — an airy, high-key fade for weddings, products, and
  lifestyle shots (similar in spirit to `lighten` mode, but fades toward the
  exact color instead of amplifying brightness).
- **sepia / brown** — vintage, old-photo warmth.
- **navy / #1A2B3C** — subtle cinematic teal-dark edges.
- Any hex works: `#RGB` shorthand (`#A52` = `#AA5522`) or `#RRGGBB`. Colors
  apply in **darken** mode; `lighten` always brightens toward white and is
  rejected with a hint if you combine it with a color.

### Limits and edge cases

- Input files up to 8 MiB; any image format ffmpeg can decode works (PNG,
  JPEG, WebP, BMP, GIF, …). The output keeps the input's exact dimensions —
  nothing is cropped or resized.
- **Output format** `keep` (default) keeps the input format; `png`, `jpg` or
  `webp` convert. Converting takes a single still frame, so an animated GIF
  keeps only its first frame — use `keep` to stay animated. `jpg` output is
  pinned to a high-quality encode (ffmpeg `-q:v 2`).
- Strength `0` is valid and returns the image unchanged; values outside
  0–100 (for strength or the two center fields) are rejected with the
  expected range named.
- The vignette is elliptical and follows the image's aspect ratio. You can
  move its center and pick its color, but its reach and softness aren't
  separately adjustable — ffmpeg's `vignette` filter couples both into the
  one strength knob, and a separate size/feather mask would be too slow to
  compute per-pixel in browser wasm for large photos.
- Transparency is not preserved: processing happens in YUV or opaque planar
  RGB, so a transparent PNG comes back fully opaque.
- Animated GIFs are processed frame by frame and stay animated (with
  format `keep`).

## FAQ

<details>
<summary>What strength should I use?</summary>

Start with the default 40 — it matches ffmpeg's classic vignette and reads as
"professionally finished" rather than "edited". Go down to 20–30 if you only
want subtle depth, and up to 60–85 for a deliberately moody look. 100 drives
the corners to pure black (or your chosen color), which works as a spotlight
effect but overwhelms most photos.

</details>

<details>
<summary>Can I make a white or light vignette instead of a dark one?</summary>

Yes, two ways. Set **Mode** to `lighten` to brighten the edges toward white —
a faded, dreamy, high-key border where at 100 the corners are fully white. Or
keep `darken` mode and set **Vignette color** to `white`: the edges then fade
toward exact white rather than being brightness-amplified, which looks softer
on already-bright photos.

</details>

<details>
<summary>Can I use a custom color, like sepia or a brand color?</summary>

Yes. **Vignette color** accepts common color names (`black`, `white`, `gray`,
`sepia`, `navy`, `red`, …) and hex values (`#A52` or `#AA5522`). The edges
fade smoothly from the untouched center toward that color; at strength 100
the far corners are exactly the color you picked. Colors apply in `darken`
mode — combining a color with `lighten` is rejected with a hint.

</details>

<details>
<summary>Can I move the vignette off-center?</summary>

Yes. **Center X** and **Center Y** place the bright spot as a percentage of
the image size — `50`/`50` is the middle, `25`/`50` centers it on the left
third, `50`/`0` on the top edge. Percentages mean the same values work for
any resolution.

</details>

<details>
<summary>Can I download the result in a different format?</summary>

Yes — set **Output format** to `png`, `jpg` or `webp` to convert while the
vignette is applied; `keep` (the default) preserves the input format. JPG
conversion uses a high-quality setting, and converting an animated GIF keeps
its first frame only.

</details>

<details>
<summary>Does the vignette crop or resize my image?</summary>

No. The output has exactly the same width and height as the input — a
vignette only changes pixel color, gradually with distance from the chosen
center. If you want to crop as well, run the image through the image-crop
tool first.

</details>

<details>
<summary>Is my photo uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the image never leaves your device.

</details>
