## Add a drop shadow to a cutout, in your browser

Pick a transparent PNG and a shadow is cast from the image's own **alpha
channel** — the silhouette of the subject, not a rectangle around it. That's
the same model CSS `filter: drop-shadow()` uses, and it's what makes a product
shot, a die-cut sticker or a logo look like it's sitting above the page
instead of pasted onto it. Everything runs locally with ffmpeg; the file never
leaves the browser.

The four knobs that matter are **Horizontal offset** and **Vertical offset**
(where the light is coming from), **Blur radius** (how far the object is from
the surface) and **Shadow opacity** (how strong the light is). By default the
canvas grows just enough for the shadow to fit, so nothing gets cut off at the
edges.

### Worked example

Take a 200×120 PNG with an opaque subject in the middle and transparency
around it, and leave every field at its default — offset `12`/`16`, blur `24`,
color `#000000`, opacity `35`, canvas margin `0` (auto):

- The output is **304×224**: the auto margin is 52 px on each side (3× the
  Gaussian reach of a 24 px blur, plus the 16 px offset), so the soft edge of
  the shadow never touches the frame.
- The far corners stay fully transparent (alpha 0) and the subject is
  untouched — still the original RGB at alpha 255. Only the shadow is blurred.
- The visible shadow band below and to the right of the subject peaks at about
  alpha 81 of 255 and falls off smoothly to 0 — roughly the 35 % opacity you
  asked for, softened by the blur.

Set **Canvas background** to `#FFFFFF` and the same run returns an opaque
white plate instead: corners become solid white and the shadow band reads as
light grey (about RGB 202) over it — a ready-to-use studio product shot.

### Choosing settings

- **Realistic product shadow** — offset `10`–`16` down, blur `24`–`40`,
  opacity `30`–`45`, black. This is roughly the default.
- **Subtle UI card** — offset `0`/`4`, blur `12`, opacity `20`. Barely
  visible, just enough to lift a card off a background.
- **Sticker / die-cut** — blur `0` and opacity `100` for a hard offset
  silhouette; add a light background color so the hard edge reads.
- **Glow** — offset `0`/`0`, a big blur (`60`–`100`) and a bright color like
  `#3366FF`. A "shadow" straight underneath with no offset is exactly a glow.
- **Light direction** — negative offsets move the shadow left (`offset_x`) and
  up (`offset_y`). A shadow up and to the left implies light from below-right.

Use the preset buttons above the form to fill all of these in one click, then
tweak from there.

### Limits and edge cases

- Input files up to 8 MiB. Any image format ffmpeg can decode works (PNG,
  WebP, JPEG, BMP, GIF, …), but only formats with an alpha channel — PNG and
  WebP, mainly — carry a cutout. **A fully opaque image (a normal JPEG) has no
  transparency, so its silhouette is the whole rectangle and you get a plain
  rectangular shadow.** Remove the background first if you want a shaped one.
- Offsets accept −500 to 500 px, blur 0 to 400 px, opacity 0 to 100 %, and the
  canvas margin 0 to 2000 px. Out-of-range values are rejected with the
  expected range named rather than being silently clamped.
- **Canvas margin** `0` means auto (blur reach + offset). Any other number is
  used verbatim on all four sides — if you set it smaller than the shadow
  needs, the shadow is clipped at the frame, which is sometimes exactly what
  you want.
- **Keep the original size** overrides the margin entirely: the output has the
  input's exact width and height, and whatever falls outside is clipped.
- Alpha in a hex color (`#RRGGBBAA`) is ignored — opacity is its own field, so
  the two can't disagree.
- `jpg` output cannot store transparency: the canvas is flattened onto the
  background color (white unless you pick one), and the encode is pinned to
  high quality (ffmpeg `-q:v 2`). Choose `png` or `webp` to keep the
  transparent background.
- Animated inputs (GIF/WebP) produce a single still frame — the first one.
- The effect is a single directional shadow. Multiple stacked shadows, spread
  (grow/shrink), and perspective "ground" shadows are not supported; running
  the tool twice composes two shadows if you need them.

## FAQ

<details>
<summary>Why is my shadow a rectangle instead of the shape of my object?</summary>

Because the image has no transparency. The shadow is cast from the alpha
channel, so an opaque photo — any JPEG, or a PNG saved with a white
background — has a silhouette that is the whole rectangle. Remove the
background first (a cutout tool, or your editor's background eraser), save as
PNG or WebP, and the shadow will follow the subject's outline.

</details>

<details>
<summary>Does the image get bigger?</summary>

Yes, by default. The canvas grows by a margin on every side so the blurred,
offset shadow always fits — with the default blur `24` and offset `16` that's
52 px per side. Set **Canvas margin** to a specific number to control it, or
tick **Keep the original size** to get back exactly the input's dimensions
(the shadow is then clipped at the frame).

</details>

<details>
<summary>What blur value should I use?</summary>

The blur radius roughly says how far the object is from the surface behind
it. `0`–`6` reads as a sticker lying flat, `12`–`24` as a card lifted slightly
off a page, `40`–`80` as an object floating well above it. It uses the same
units as CSS `drop-shadow()` — internally the Gaussian sigma is half the
radius — so a value copied from a CSS `box-shadow` will look about the same
here.

</details>

<details>
<summary>Can I make a colored glow instead of a shadow?</summary>

Yes. Set both offsets to `0`, pick a bright shadow color, raise the opacity
and use a large blur. With nothing offsetting it, the blurred silhouette
spreads evenly in all directions, which is exactly what a glow is. `#3366FF`
at opacity `70` with blur `80` is a good starting point — the "Colored glow"
preset does it in one click.

</details>

<details>
<summary>Why does the shadow look weaker than the opacity I set?</summary>

Opacity sets the strength of the shadow *before* it is blurred. Blurring
spreads that alpha out, so the visible edge of a soft shadow is always lighter
than the number you typed, and only the fully-covered middle — usually hidden
behind the subject — reaches it. Lower the blur or raise the opacity if you
want more presence; opacity `100` with blur `0` gives the exact color at full
strength.

</details>

<details>
<summary>Is anything uploaded?</summary>

No. The page loads an ffmpeg build compiled to WebAssembly and runs the whole
filter chain on your machine, so the image is never sent anywhere. The same
tool is available on the command line if you'd rather script it over a folder
of assets — the CLI example above is generated from this page's settings.

</details>
