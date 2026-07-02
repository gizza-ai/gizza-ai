## Normalize audio loudness in your browser

Pick an audio file and a target loudness — the tool levels it with ffmpeg's
EBU R128 `loudnorm` filter (single pass, true peak capped at -1.5 dBTP,
loudness range 11 LU), entirely in your browser. Nothing is uploaded, so it's
safe for unreleased tracks and private recordings.

### Worked example

Prepare a podcast episode: upload `episode.wav`, set **Target loudness** to
`-16` (the podcast standard) and **Format** to `mp3`. The result is
`episode-normalized.mp3` at a consistent loudness — quiet passages lifted,
peaks kept under -1.5 dBTP — ready to publish without listeners riding the
volume knob.

### Common targets

- **-14 LUFS** — Spotify, YouTube, most streaming platforms (the default).
- **-16 LUFS** — podcasts and Apple Music.
- **-23 LUFS** — EBU R128 broadcast (EU); use **-24** for US ATSC A/85.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works.
- Accepted target range is **-70 to -5 LUFS** (the `loudnorm` filter's limits).
- Single-pass loudnorm normalizes dynamically as it goes — for very dynamic
  material a dedicated two-pass workflow can land ~1 LU closer to the exact
  target, but single pass is what online tools use and is right for voice and
  music alike.
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC).
  Embedded album art is dropped.

## FAQ

<details>
<summary>Which LUFS target should I pick?</summary>

Match your destination: `-14` for Spotify/YouTube uploads, `-16` for podcasts
and Apple Music, `-23` for EU broadcast delivery. When in doubt, leave the
default — over-loud files just get turned down by the platform anyway.

</details>

<details>
<summary>Will normalizing make my quiet recording louder?</summary>

Yes — that's the main use. loudnorm raises the overall level to the target
while keeping true peaks below -1.5 dBTP, so a too-quiet voice memo comes out
at a consistent, comfortable volume without clipping.

</details>

<details>
<summary>Why does my file sound slightly compressed afterwards?</summary>

Single-pass loudnorm adjusts gain dynamically, which gently reduces the
difference between the loudest and quietest parts (loudness range 11 LU).
For spoken word that's desirable; for very dynamic classical music, consider
mastering offline with a two-pass workflow instead.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
