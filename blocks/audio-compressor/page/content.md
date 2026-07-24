## Compress audio dynamic range in your browser

Pick an audio file and set the four classic compressor controls — **threshold**,
**ratio**, **attack** and **release** — plus optional **make-up gain**. The tool
runs ffmpeg's `acompressor` filter entirely in your browser to pull loud and
quiet passages closer together, so the audio sits at a steadier, more consistent
level. Nothing is uploaded, so it's safe for unreleased tracks and private
recordings.

This is **dynamics** compression (evening out the level), not **file-size**
compression — to make a file smaller pick a lower bitrate with the
audio-compress tool instead.

### Worked example

A voice-over `narration.wav` where some lines jump out and others disappear:
upload it, set **Threshold** to `-24`, **Ratio** to `4`, **Attack** to `10`,
**Release** to `150` and **Make-up gain** to `6`. The result
`narration-compressed.mp3` holds a tight, even level — the quiet lines come
forward, the loud ones stop spiking, and the 6 dB make-up gain brings the whole
thing back up to a comfortable volume.

### What each control does

- **Threshold (dB, −60…0)** — the level where compression kicks in. Lower (−30)
  catches more of the signal; higher (−10) only tames the loudest peaks.
- **Ratio (1…20)** — how hard everything above the threshold is squeezed. `2` is
  gentle, `4` is a firm even level, `10`+ approaches limiting. `1` is no
  compression.
- **Attack (ms, 0.01…2000)** — how fast it clamps down once the signal passes
  the threshold. Fast (`5`) tames sharp transients; slow (`30`+) lets punch
  through.
- **Release (ms, 0.01…9000)** — how fast it lets go once the signal drops back.
  Short (`60`) for speech, longer (`250`+) for smoother music.
- **Make-up gain (dB, 0…24)** — level added *after* compression to restore the
  loudness the compressor pulled down. Try `3`–`6` when heavier settings leave
  the result too quiet.

### Starting points

- **Voice / podcast** — threshold `-24`, ratio `4`, attack `10`, release `150`,
  make-up `4`–`6`. Even, forward, intelligible.
- **Music glue** — threshold `-18`, ratio `2`–`3`, attack `30`, release `250`.
  Gentle cohesion without squashing the dynamics.
- **Peak taming** — threshold `-10`, ratio `8`+, fast attack `5`, make-up `0`.
  Only the loudest moments get caught.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works (mp3, wav, m4a,
  ogg, flac, and more).
- A **ratio of 1 with 0 dB make-up gain does nothing** and is rejected — raise
  the ratio above 1 to compress, or add make-up gain to use the tool as a clean
  level boost.
- Output is re-encoded because the compressor rewrites the samples (mp3/ogg at
  192 kbps; wav/flac lossless; m4a AAC). Embedded album art is dropped.
- Compression is a creative, one-way change — too much (very high ratio + heavy
  make-up) can sound flat and pumpy, so keep the original if it matters.

## FAQ

<details>
<summary>What's the difference between this and the "audio compress" tool?</summary>

They do different things. This **audio compressor** does *dynamic-range*
compression — it evens out the loud and quiet parts so the level stays steady,
using threshold/ratio/attack/release. The separate **audio-compress** tool does
*file-size* compression — it re-encodes at a lower bitrate to make the file
smaller. Reach for this one to control loudness, and audio-compress to shrink a
file.

</details>

<details>
<summary>What threshold and ratio should I start with?</summary>

For speech and podcasts, threshold `-24` dB with ratio `4:1` gives a firm, even
level; add `4`–`6` dB of make-up gain to bring it back up. For gentle "glue" on
music, use a higher threshold (`-18`) and a lower ratio (`2`–`3`). To only catch
the loudest peaks, raise the threshold toward `-10` and the ratio to `8`+.

</details>

<details>
<summary>Why does my compressed audio sound quieter?</summary>

Compression turns the loud parts down, so the overall level often drops — that's
what **make-up gain** is for. Add a few dB (start with `3`–`6`) to restore the
loudness the compressor pulled down. With enough make-up gain the result can end
up louder *and* more even than the original.

</details>

<details>
<summary>What do attack and release actually change?</summary>

**Attack** is how quickly the compressor reacts once the signal crosses the
threshold: a fast attack (`5` ms) clamps sharp transients like plosives or drum
hits, while a slower one (`30`+ ms) lets that initial punch through before
compressing. **Release** is how quickly it stops compressing afterwards: short
releases (`60` ms) suit speech, longer ones (`250`+ ms) keep music smooth and
avoid audible pumping.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file locally in the browser tab — the audio never leaves your device.

</details>
