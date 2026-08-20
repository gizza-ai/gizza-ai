## What a median filter does

A median filter slides a small square window over the image and replaces the
centre pixel with the **middle value** of everything inside that window. Because
the middle value is a real pixel from the neighbourhood — not an average — a
lone white or black dot has no influence at all: it sorts to the end of the list
and is thrown away, while the edge running through the window keeps its two
populations of pixels and stays exactly where it was.

That is why median filtering is the standard answer to **salt-and-pepper noise**
(scattered black/white dots from bad sensor pixels, transmission errors or a
dusty scan) and why it beats a Gaussian blur, which smears the dot into a grey
smudge and softens every edge along with it.

### Worked example

Take a 3-pixel-wide row of a photo where one pixel has been hit by noise:

```
neighbours: 118  120  255  121  119
sorted:     118  119  120  121  255
median:                120
```

The blown-out `255` is discarded and the pixel becomes `120`, matching its
neighbours. An average would have produced `146` — a visible bright smudge.

On this page: upload a noisy image, leave **Window radius** at `1` (a 3×3
window) and press run. Single-pixel dots are gone and the result downloads in
the same format you uploaded. Raise the radius to `2` (5×5) for scanner dust or
speckle that covers a couple of pixels.

### The controls

- **Window radius** — the window is `2 × radius + 1` square. `1` = 3×3,
  `2` = 5×5, `5` = 11×11, `20` = 41×41. Bigger windows erase bigger blobs of
  noise but also erase fine texture, and the filter gets quadratically slower.
- **Specks to remove** — *both* is the true median. *Bright only* leans toward
  the darker neighbours, so white dust is wiped harder (the image darkens a
  little); *dark only* does the reverse for black specks on light paper.
- **Channels to filter** — *all* cleans everything. *Luma only* smooths
  brightness noise while colour detail is untouched. *Chroma only* removes the
  blotchy red/green patches of high-ISO colour noise and leaves luminance detail
  pin-sharp — usually the best-looking option for photos.
- **Passes** — running a small window twice removes dense noise more gently than
  one large window, because each pass only ever moves a pixel to a value that
  already exists nearby.
- **Output format / quality** — *keep* returns the format you uploaded; PNG is
  lossless; JPG and WebP use the quality slider (1–100, default 92).
- **Strip metadata** — drops EXIF, camera info and comments from the output.

### Limits and edge cases

- Input files up to **8 MB**, any format your browser's decoder handles (PNG,
  JPG, WebP, GIF, BMP, TIFF).
- Radius is capped at **20** (a 41×41 window) and passes at **3**. Large radius
  values on a multi-megapixel photo can take a while — the work grows with the
  square of the window.
- The median filter removes *impulse* noise. Fine film grain or heavy Gaussian
  sensor noise is better handled by a dedicated grain/denoise pass; a median
  filter will just flatten texture.
- Text, thin lines and small dots are also "isolated pixels": at radius 3 and
  above, fine line art and small type start to erode. Keep the window as small
  as it can be while still catching the noise.
- *Luma only* and *chroma only* convert the image through Y'CbCr, which can
  shift untouched channels by ±1 sRGB step. *All* filters the image in its
  native planar layout, with no colour conversion.
- Choosing PNG, JPG or WebP for an animated GIF keeps only the first frame;
  *keep* filters every frame and stays animated.

## FAQ

<details>
<summary>What is the difference between a median filter and a blur?</summary>

A blur averages the pixels in the window, so a bright noise dot contaminates
every pixel around it and edges soften. A median *selects* a value that already
exists in the window, so an outlier is discarded outright and a straight edge
stays straight. For salt-and-pepper noise the median wins clearly; for smooth
grain, a blur or a dedicated denoiser is a better fit.

</details>

<details>
<summary>What radius should I use?</summary>

Start at `1` (a 3×3 window) — it removes single-pixel dots with almost no loss
of detail. Use `2` (5×5) if the specks are 2–3 pixels wide, such as dust on a
scan. Above `4` the image starts to look like a watercolour, so if you need that
much cleaning, try 2 passes at a small radius instead of one big window.

</details>

<details>
<summary>Why does the result look flat or "painted"?</summary>

Because the window is too large for the detail in the image. Every pass replaces
texture with the local middle value, so fine grain, skin texture and small type
get flattened. Lower the radius, drop to a single pass, or switch **Channels to
filter** to *chroma only* so luminance detail is left completely untouched.

</details>

<details>
<summary>Can it clean up a scanned document?</summary>

Yes — that is the classic use. A radius of 1–2 with *dark only* removes pepper
specks from light paper without eating the strokes of the text, and *bright
only* removes white pinholes from a dark background. Choose PNG output so the
cleaned scan is not re-compressed, and tick **Strip metadata** if you are about
to publish it.

</details>

<details>
<summary>Are my images uploaded anywhere?</summary>

No. The filter runs in your browser through a WebAssembly build of ffmpeg. The
file you pick is read locally, processed locally, and the download link points
at the result held in your own tab's memory.

</details>
