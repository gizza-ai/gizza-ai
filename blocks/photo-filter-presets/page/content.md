## About photo filter presets

Photo Filter Presets applies a one-click, film-inspired look to any image —
entirely in your browser. Pick a picture, choose a preset, and the filter is
rendered locally with WebAssembly ffmpeg. Nothing is uploaded to a server, so
your photos stay on your device and it keeps working offline once the page has
loaded.

## The presets

- **Sepia** — warm brown monochrome for a classic, aged-photograph feel.
- **Vintage** — faded contrast with a warm cast, like old print film.
- **Warm** — shifts the white balance toward amber for a cozy tone.
- **Cool** — shifts toward blue for a crisp, wintry look.
- **Noir** — high-contrast black and white for a dramatic, moody shot.
- **Grayscale** — a neutral black-and-white conversion with no tint.
- **Vivid** — boosts saturation and contrast to make colors pop.
- **Invert** — flips every color to its opposite (a photo negative).
- **Fade** — lifts the blacks for a soft, low-contrast matte finish.

## How to use it

1. Choose an image (JPG, PNG, WebP, and other common formats).
2. Pick a preset from the dropdown.
3. The filtered image renders in place — download it from the link below the preview.

## FAQ

<details>
<summary>Are my photos uploaded anywhere?</summary>

No. The filter runs locally in your browser with WebAssembly ffmpeg; the image
never leaves your device, and the tool keeps working offline.

</details>

<details>
<summary>What image formats can I use?</summary>

Any common raster format your browser can read — JPG, PNG, WebP, GIF, or BMP.
The result is a filtered image you can download.

</details>

<details>
<summary>What's the difference between noir and grayscale?</summary>

Both are black and white. **Grayscale** is a plain, neutral conversion, while
**noir** adds strong contrast for a darker, more cinematic look.

</details>

<details>
<summary>Can I use it from the command line or by URL?</summary>

Yes — every preset is available via `gizza tool photo-filter-presets` and by URL
query parameter (`?preset=sepia`), so you can script it or deep-link a specific
filter.

</details>
