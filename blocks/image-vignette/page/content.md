## Add a vignette to a photo, in your browser

Pick an image and a strength — a vignette is applied with ffmpeg, entirely in
your browser. A vignette darkens the image gradually toward the edges and
corners, pulling the viewer's eye to the middle of the frame; it's the classic
finishing touch for portraits, product shots, and moody landscapes. Switch
**Mode** to `lighten` to brighten the edges instead for a faded, hazy border.
Under the hood the friendly **Strength** value (0–100) is mapped onto the
ffmpeg `vignette` filter's angle — the default `40` lands exactly on the
filter's classic default, and you never have to think in radians.

### Worked example

Upload `portrait.jpg` and leave **Strength** at `40` — the result keeps the
same dimensions, the face in the middle stays at full brightness, and the
corners fall off softly. In numbers: on a plain white test image, strength
`40` leaves the center at RGB 255 while the far corners drop to roughly
RGB 110; at strength `80` the corners are nearly black (about RGB 3), and at
`100` they reach pure black. If your subject sits on the left third, set
**Center X** to `25` — the bright spot follows the subject and the right side
darkens more.

### Picking a strength

- **15–30** — barely-there edge falloff; adds depth without being noticed.
- **35–55** — the classic photographic vignette; the default 40 sits here.
- **60–85** — dramatic, moody edges for portraits and posters.
- **90–100** — corners go fully black (or white in lighten mode); a
  spotlight/tunnel effect rather than a subtle finish.

### Limits and edge cases

- Input files up to 8 MiB; any image format ffmpeg can decode works (PNG,
  JPEG, WebP, BMP, GIF, …). The output keeps the input format and its exact
  dimensions — nothing is cropped or resized.
- Strength `0` is valid and returns the image unchanged; values outside
  0–100 (for strength or the two center fields) are rejected.
- The vignette is elliptical and follows the image's aspect ratio. You can
  move its center, but its shape, softness and color aren't separately
  adjustable — strength controls how dark and how far in the falloff reaches.
- Transparency is not preserved: processing happens in a YUV colorspace, so
  a transparent PNG comes back fully opaque.
- Animated GIFs are processed frame by frame and stay animated.

## FAQ

<details>
<summary>What strength should I use?</summary>

Start with the default 40 — it matches ffmpeg's classic vignette and reads as
"professionally finished" rather than "edited". Go down to 20–30 if you only
want subtle depth, and up to 60–85 for a deliberately moody look. 100 drives
the corners to pure black, which works as a spotlight effect but overwhelms
most photos.

</details>

<details>
<summary>Can I make a white or light vignette instead of a dark one?</summary>

Yes — set **Mode** to `lighten`. Instead of darkening, the edges are
brightened toward white, which gives a faded, dreamy, high-key border. The
strength scale works the same way: at 100 the corners are fully white.

</details>

<details>
<summary>Can I move the vignette off-center?</summary>

Yes. **Center X** and **Center Y** place the bright spot as a percentage of
the image size — `50`/`50` is the middle, `25`/`50` centers it on the left
third, `50`/`0` on the top edge. Percentages mean the same values work for
any resolution.

</details>

<details>
<summary>Does the vignette crop or resize my image?</summary>

No. The output has exactly the same width and height as the input — a
vignette only changes pixel brightness, gradually with distance from the
chosen center. If you want to crop as well, run the image through the
image-crop tool first.

</details>

<details>
<summary>Is my photo uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the image never leaves your device.

</details>
