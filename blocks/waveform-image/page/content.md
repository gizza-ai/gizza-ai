## Make a waveform image from any audio file

Upload an audio file and this tool renders its waveform as a PNG image —
the classic soundwave graphic used for podcast covers, social banners,
audio-post thumbnails and player artwork. Choose the image size, a solid or
gradient wave color and an optional background color, or leave the background
empty for a transparent PNG that drops onto any design. The render runs
entirely in your browser with ffmpeg compiled to WebAssembly (its
`showwavespic` filter), so your audio is never uploaded to a server.

### Worked example

Make a social banner for a podcast episode: upload the episode mp3, leave
**Width** and **Height** empty (the default is a banner-shaped `1200×300`),
set **Wave color** to `#4f46e5` and **Background** to `#0b1220`. The result is
a 1200×300 PNG named like the original with a `-waveform.png` suffix — dark
card, indigo wave. Add `#ec4899` as the **Gradient end** and the wave fades
indigo→pink left to right. For a transparent overlay to place on top of your
own artwork, leave **Background** empty instead: the PNG keeps its alpha
channel, so only the wave itself is opaque. A quiet voice memo that renders as
a thin line becomes clearly visible with **Amplitude scale** set to `sqrt` or
`log` — and **Sampling** set to `peak` keeps short hits visible. Or just tap
one of the **Try** presets under the fields.

### Options

- **Width / Height** — image size in pixels (16–4096 × 16–2048). Empty fields
  use the 1200×300 default.
- **Wave color** — hex color: `#RGB`, `#RRGGBB` or `#RRGGBBAA` (the last two
  digits are alpha, for a translucent wave). Pick it with the swatch or type
  it. With **One lane per channel**, a comma-separated list like
  `#4f46e5,#ec4899` colors each channel's lane (up to 8).
- **Gradient end** — a second hex color. When set, the wave is filled with a
  horizontal left→right gradient from **Wave color** to this color. Leave
  empty for a solid wave.
- **Background** — hex color, or empty for a transparent PNG. An alpha hex
  like `#00000080` bakes in a see-through scrim.
- **One lane per channel** — off (default) downmixes to a single clean mono
  wave; on draws each channel (e.g. stereo left/right) in its own horizontal
  lane, sharing the image height.
- **Amplitude scale** — `lin` is the true waveform; `sqrt`, `cbrt` and `log`
  progressively boost quiet material so it stays visible.
- **Sampling** — `average` (default) draws the mean level of each pixel
  column; `peak` draws the loudest sample instead, a fuller wave that keeps
  drum hits and other transients visible.

### Limits and edge cases

- Input files up to 10 MiB; anything ffmpeg can decode works (mp3, wav, flac,
  m4a/aac, ogg, opus — and most video containers' audio tracks via the CLI).
- The output is always a PNG (with an alpha channel when the background is
  empty). Colors must be hex — named colors like `red` are rejected with a
  hint rather than guessed. Short `#f00`-style hex is expanded for you.
- A gradient (**Gradient end** set) needs a single **Wave color**, not a
  comma list — the gradient runs across the whole image, so split-channel
  lanes all share it.
- Very quiet recordings can look like a flat line in `lin` scale — that's the
  honest amplitude, not a bug; switch the scale to `sqrt` or `log`, or set
  **Sampling** to `peak`.
- A silent file renders only the thin center baseline.
- One image per run: the whole track is drawn into the width you choose, one
  peak column per horizontal pixel — longer audio isn't wider, it's just more
  compressed.

## FAQ

<details>
<summary>How do I get a transparent background?</summary>

Leave the **Background** field empty (the default). The PNG then keeps its
alpha channel, so everything except the wave itself is fully transparent —
ideal for dropping the waveform onto your own artwork or a colored card in
any editor. Set a hex value like `#0b1220` only when you want a flat
background baked into the image, or an alpha hex like `#00000080` for a
half-transparent scrim.

</details>

<details>
<summary>Can the wave be a gradient?</summary>

Yes — set **Gradient end** to a second hex color. The wave is then filled
with a smooth horizontal gradient that starts at **Wave color** on the left
and ends at **Gradient end** on the right (try `#f97316` → `#ec4899` for a
sunset look). Everything else — transparent or solid backgrounds, channel
lanes, scales — works the same as with a solid color.

</details>

<details>
<summary>Why does my quiet recording show almost no waveform?</summary>

The default `lin` scale draws true amplitude, and quiet audio genuinely has
small peaks. Switch **Amplitude scale** to `sqrt` — or `log` for the
strongest boost — to make quiet material fill more of the image height
without touching the audio itself. Setting **Sampling** to `peak` helps too:
it draws each column at the loudest sample instead of the average, so short
transients stop disappearing.

</details>

<details>
<summary>What size should I pick for social media or a podcast player?</summary>

The default `1200×300` works well for wide banners, link previews and
audio-post headers. For a square post, try `1080×1080`; for a compact player
strip, something like `800×160`. Any size from 16×16 up to 4096×2048 works —
the whole track is always drawn across the full width. The **Try** presets
under the fields cover the common shapes.

</details>

<details>
<summary>Can I show the left and right channels separately?</summary>

Yes — tick **One lane per channel**. A stereo file then renders two stacked
waves (left on top, right below) sharing the image height, and a
comma-separated **Wave color** list like `#4f46e5,#ec4899` gives each lane
its own color. Off, the channels are downmixed to one mono wave, which is
usually the cleaner look for artwork.

</details>

<details>
<summary>Can I make bar-style or circular waveforms?</summary>

Not with this tool — it draws the classic continuous waveform (ffmpeg's
`showwavespic`), in solid or gradient color. Bar, dotted and radial
"sound-wave art" styles need a different renderer and aren't supported here.
What you can vary: size, colors (including alpha), gradient, background,
per-channel lanes, amplitude scale and peak sampling.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then renders the
image locally in the browser tab — the audio never leaves your device.

</details>
