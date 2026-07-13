## Adjust a video's brightness, contrast & color

Pick a video and drag four sliders — **brightness**, **contrast**, **saturation**,
and **gamma** — to correct exposure, add punch, mute the colors, or drop to
grayscale. The clip is re-encoded right in your browser with ffmpeg's `eq`
filter; nothing is uploaded.

### What each slider does

- **Brightness** (`-1` to `1`, default `0`) — shifts every pixel lighter or
  darker. `0` leaves it unchanged.
- **Contrast** (`0` to `4`, default `1`) — spreads or compresses the tonal
  range. `1` is unchanged, `0` collapses to flat gray, above `1` is punchier.
- **Saturation** (`0` to `3`, default `1`) — color intensity. `1` is unchanged,
  `0` is grayscale, above `1` is more vivid.
- **Gamma** (`0.1` to `10`, default `1`) — midtone brightness curve. `1` is
  unchanged; below `1` lifts the midtones (brighter shadows), above `1` darkens
  them.

Leaving all four at their defaults (`0 / 1 / 1 / 1`) is the identity — the video
comes out visually unchanged (just re-encoded).

### Worked example

A slightly flat clip with `brightness = 0.1`, `contrast = 1.2`,
`saturation = 1.4`, `gamma = 0.9` comes out brighter, punchier, and more vivid.
Under the hood that is a single ffmpeg pass:
`eq=brightness=0.1:contrast=1.2:saturation=1.4:gamma=0.9`.

### Notes

- The output is re-encoded to **H.264 video + AAC audio**. An `.mp4`, `.mov`,
  `.m4v`, or `.mkv` keeps its container; anything else (a `.webm`, …) comes out
  as **MP4**.
- Max input size is **25 MB**. Adjusting re-encodes the whole video, so large
  files take longer — everything runs locally on your machine.
- Need to rotate a hue instead of these four channels? That's a separate
  operation (ffmpeg's `hue` filter) and isn't part of this tool.

<details>
<summary>What values leave the video unchanged?</summary>

The identity is **brightness `0`, contrast `1`, saturation `1`, gamma `1`** —
that's the default. At those settings the video is only re-encoded, with no
visible color change. Move any slider away from its default to see an effect.

</details>

<details>
<summary>How do I make the video grayscale?</summary>

Set **saturation to `0`**. That strips all color while keeping brightness and
contrast. Combine it with a small contrast bump (say `1.1`) for a punchier
black-and-white look.

</details>

<details>
<summary>Does adjusting the color change the format or quality?</summary>

The streams are re-encoded to **H.264 video + AAC audio** — color adjustment
can't be applied without re-encoding. An `.mp4`/`.mov`/`.m4v`/`.mkv` keeps its
container; other inputs are converted to **MP4** because those containers can't
hold H.264/AAC. Re-encoding adds a small generational quality loss, so adjust
once from the original rather than repeatedly re-processing outputs.

</details>

<details>
<summary>Is my video uploaded to a server?</summary>

No — the ffmpeg engine runs inside your browser tab, so the file never leaves
your device.

</details>

<details>
<summary>Why is contrast capped at 4 and gamma at 10?</summary>

Those are the useful ranges of ffmpeg's `eq` filter for these controls. Beyond
them the picture clips to solid blocks with no extra detail, so the sliders stop
at brightness `±1`, contrast `4`, saturation `3`, and gamma `10`.

</details>
