## Make a waveform image from any audio file

Upload an audio file and this tool renders its waveform as a PNG image —
the classic soundwave graphic used for podcast covers, social banners,
audio-post thumbnails and player artwork. Choose the image size, the wave
color and an optional background color, or leave the background empty for a
transparent PNG that drops onto any design. The render runs entirely in your
browser with ffmpeg compiled to WebAssembly (its `showwavespic` filter), so
your audio is never uploaded to a server.

### Worked example

Make a social banner for a podcast episode: upload the episode mp3, leave
**Width** and **Height** empty (the default is a banner-shaped `1200×300`),
set **Wave color** to `#4f46e5` and **Background** to `#0b1220`. The result is
a 1200×300 PNG named like the original with a `-waveform.png` suffix — dark
card, indigo wave. For a transparent overlay to place on top of your own
artwork, leave **Background** empty instead: the PNG keeps its alpha channel,
so only the wave itself is opaque. A quiet voice memo that renders as a thin
line becomes clearly visible with **Amplitude scale** set to `sqrt` or `log`.

### Options

- **Width / Height** — image size in pixels (16–4096 × 16–2048). Empty fields
  use the 1200×300 default.
- **Wave color** — `#RGB` or `#RRGGBB` hex, e.g. `#4f46e5` or `#f00`.
- **Background** — hex color, or empty for a transparent PNG.
- **One lane per channel** — off (default) downmixes to a single clean mono
  wave; on draws each channel (e.g. stereo left/right) in its own horizontal
  lane, sharing the image height.
- **Amplitude scale** — `lin` is the true waveform; `sqrt`, `cbrt` and `log`
  progressively boost quiet material so it stays visible.

### Limits and edge cases

- Input files up to 10 MiB; anything ffmpeg can decode works (mp3, wav, flac,
  m4a/aac, ogg, opus — and most video containers' audio tracks via the CLI).
- The output is always a PNG (with an alpha channel when the background is
  empty). Colors must be strict hex — named colors like `red` are rejected
  with a hint rather than guessed.
- Very quiet recordings can look like a flat line in `lin` scale — that's the
  honest amplitude, not a bug; switch the scale to `sqrt` or `log`.
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
background baked into the image.

</details>

<details>
<summary>Why does my quiet recording show almost no waveform?</summary>

The default `lin` scale draws true amplitude, and quiet audio genuinely has
small peaks. Switch **Amplitude scale** to `sqrt` — or `log` for the
strongest boost — to make quiet material fill more of the image height
without touching the audio itself.

</details>

<details>
<summary>What size should I pick for social media or a podcast player?</summary>

The default `1200×300` works well for wide banners, link previews and
audio-post headers. For a square post, try `1080×1080`; for a compact player
strip, something like `800×160`. Any size from 16×16 up to 4096×2048 works —
the whole track is always drawn across the full width.

</details>

<details>
<summary>Can I show the left and right channels separately?</summary>

Yes — tick **One lane per channel**. A stereo file then renders two stacked
waves (left on top, right below) sharing the image height. Off, the channels
are downmixed to one mono wave, which is usually the cleaner look for
artwork.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then renders the
image locally in the browser tab — the audio never leaves your device.

</details>
