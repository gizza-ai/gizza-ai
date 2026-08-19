## About this tool

Dithering reduces an image to a small palette **without** the flat banding you get
from plain colour quantization. Instead of snapping each pixel to the nearest
palette colour and discarding the difference, a dithering algorithm spreads that
error into neighbouring pixels, so a fine pattern of dots stands in for the
colours the palette no longer has. Step back from the result and your eye blends
the dots into a continuous tone again — which is why a photo reduced to pure black
and white is still perfectly readable.

That trick is behind three different jobs this tool does:

- **Retro / pixel-art looks.** Old hardware had four to sixteen colours, and
  dithering is the texture that defines that era's images.
- **E-ink and embedded displays.** E-paper panels are 1-bit or 4/16-level
  grayscale. Dithering to the panel's exact ramp before you flash an image is
  what makes photos look like photos on it.
- **Palette reduction in general.** Fewer colours means smaller files, and for
  flat artwork a dithered 16-colour PNG or GIF can be a fraction of the original.

Everything runs locally in your browser — the image is never uploaded anywhere.

### Worked example

Take a 640 x 480 colour photograph and prepare it for a 1-bit e-paper badge:

| Setting | Value |
| --- | --- |
| Dither algorithm | `atkinson` |
| Palette | `mono` |
| Contrast before dithering | `1.4` |
| Output format | `png` |

The result is still 640 x 480, but it now contains **exactly two colours**,
`#000000` and `#ffffff` — nothing in between. Where the photo had mid-grey, the
output has a fine stipple of black and white dots whose local density matches the
original brightness. Bumping contrast to 1.4 first stops flat, hazy regions from
collapsing into an even grey mush, which is the single most common problem when
dithering to two colours.

The same photo with **Palette: auto** and **Palette size: 16** keeps its colour,
but every pixel is now one of at most sixteen colours picked from the photo
itself, with Floyd-Steinberg dots hiding the transitions.

### The algorithms

**Error diffusion** kernels push each pixel's quantization error onto pixels not
yet processed. They differ in how far and how heavily they spread it:

- `floyd_steinberg` — the default and the one most people mean by "dithering".
  Spreads to four neighbours; fine-grained and detailed.
- `atkinson` — diffuses only three quarters of the error, so highlights and
  shadows clip to flat white and black. Sparse, punchy, very recognisable.
- `burkes`, `sierra2`, `sierra3`, `sierra2_4a` — wider or narrower kernels,
  running from smoothest (`sierra3`) to grainiest and fastest (`sierra2_4a`).
- `heckbert` — a simple, small error-diffusion kernel.

**Ordered dithering** (`bayer`) is different: it compares each pixel against a
fixed threshold matrix rather than looking at its neighbours. That makes it
deterministic and tileable, and gives the flat, regular crosshatch familiar from
print and early games. **Ordered matrix coarseness** (0–5) sets how large that
pattern is — 0 is a tight fine grain, 5 is a big visible weave.

`none` skips dithering altogether. It is there for comparison: run it once to see
the banding, then switch back.

### Palettes

- **Auto** derives a palette from your image with up to **Palette size**
  entries (2–256). This is the one to use when you want to keep the image's own
  colours.
- **Mono** is exactly `#000000` and `#ffffff` — true 1-bit.
- **Grayscale 4 / 16 levels** are evenly spaced black-to-white ramps matching
  common e-paper panels.
- **Green LCD (4 shades)** and **Amber terminal (2 shades)** are single-hue retro
  looks. **CGA (4 colours)** is the classic four-colour display palette.
- **Custom** takes your own comma-separated hex list, 2–16 colours, in either
  `#rgb` or `#rrggbb` form — e.g. `#1b1b1b,#e8e8e8` or
  `000000,ff5555,55ffff,ffffff`.

Single-hue palettes (mono, both grayscales, green LCD, amber) convert the image to
brightness first, so a saturated red maps to the shade that matches how *bright*
it looks rather than whichever palette entry happens to sit nearest in the RGB
cube.

### Chunky pixels and contrast

**Chunky pixel size** (1–16) scales the image down by that factor with
nearest-neighbour, dithers it, then scales it back up, so each dithered dot
becomes an N x N block. The output keeps its original dimensions; only the
apparent resolution drops. 4–8 gives a convincing pixel-art look.

**Contrast before dithering** (0.5–3.0) applies a contrast adjustment first. At
small palettes this matters far more than it sounds: 1.3–1.8 is often the
difference between a legible 1-bit image and grey soup.

### Limits and edge cases

- Input images up to **16 MB**. Very large images are slow to dither in the
  browser; downscale first if a run takes too long.
- **Output format matters.** PNG (the default), GIF and WebP are written
  losslessly and preserve the dither pattern exactly. JPEG is offered but will
  visibly smear single-pixel dots into blur and colour fringes — the pattern is
  precisely the kind of high-frequency detail JPEG throws away.
- **Do not resize a dithered image afterwards.** Any interpolation blends the
  dots back into intermediate colours and destroys the effect. Set the size you
  want *before* dithering (or use chunky pixel size).
- **Palette size only affects the auto palette.** For every fixed palette the
  number of colours is set by the palette itself, and the slider is ignored.
- **Custom palettes accept 2–16 colours.** More than that and the auto palette
  with a matching palette size is the better tool.
- Very small images make the algorithms hard to tell apart — differences show up
  at a few hundred pixels and above.
- Animated GIFs are treated as a single still frame; this tool dithers images,
  not animations.

## FAQ

<details>
<summary>What is the difference between Floyd-Steinberg and ordered (Bayer) dithering?</summary>

Floyd-Steinberg is **error diffusion**: each pixel is rounded to the nearest
palette colour and the leftover error is pushed onto neighbouring pixels that
haven't been processed yet. The dot pattern therefore depends on the image
content and looks organic and irregular.

Ordered dithering compares each pixel against a fixed threshold matrix instead.
Nothing is passed between pixels, so the result is a regular, repeating
crosshatch — deterministic, tileable, and the look most associated with print
screens and early game graphics. Use `floyd_steinberg` when you want detail,
`bayer` when you want a visible, uniform texture.

</details>

<details>
<summary>Why does my dithered image look like grey mush?</summary>

Almost always because the source is flat or hazy and the palette is very small.
When most of the image sits in a narrow band of mid-tones, a 2-colour palette has
nothing to work with and produces an even 50/50 stipple everywhere.

Raise **Contrast before dithering** to about 1.3–1.8 and re-run. That pushes the
tones apart before the dithering happens and usually restores the shapes
immediately. If it still looks flat, try `atkinson`, which deliberately clips
highlights and shadows to pure white and black.

</details>

<details>
<summary>Which settings should I use for an e-ink or e-paper display?</summary>

Match the panel. A 1-bit black-and-white panel wants **Palette: mono**; a
grayscale panel wants **gray4** or **gray16** depending on how many levels it
supports. Use `atkinson` or `floyd_steinberg`, raise contrast a little
(1.2–1.5), and export as **PNG** so no compression touches the dots.

Resize the image to the panel's exact pixel dimensions *before* dithering — a
dithered image that gets rescaled on the way to the display loses the whole
effect.

</details>

<details>
<summary>Why is PNG the default instead of JPEG?</summary>

A dither pattern is single-pixel, maximum-frequency detail, which is exactly what
JPEG's compression discards first. A dithered image saved as JPEG comes back
blurred, with colour fringing around the dots and often more distinct colours than
the palette you asked for.

PNG, GIF and WebP are written losslessly here, so the output contains exactly the
palette colours and exactly the pattern that was computed. JPEG is still offered
for when file size wins, but expect the pattern to soften.

</details>

<details>
<summary>Does the palette size slider do anything with a fixed palette?</summary>

No. **Palette size** only applies when **Palette** is set to `auto`, where it caps
how many colours are extracted from the image. Every other palette — mono, the
grayscale ramps, the retro palettes, and your custom list — defines its own
colours, so the slider is ignored.

If you want a specific number of image-derived colours, use `auto` and set the
slider. If you want specific *colours*, use `custom` and list them.

</details>

<details>
<summary>How do I get a chunky pixel-art look rather than fine dots?</summary>

Raise **Chunky pixel size**. At 1 (the default) the dither dots are single
pixels. At 6, the image is reduced to a sixth of its resolution, dithered there,
and blown back up with nearest-neighbour, so every dot is a crisp 6 x 6 block and
the output still has its original dimensions.

Pair it with a small fixed palette — `green4`, `cga4`, or a custom four-colour
list — and a contrast bump around 1.3 for the strongest retro result.

</details>
