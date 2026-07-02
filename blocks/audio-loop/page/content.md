## Loop audio in your browser

Pick a short sound and how long the result should be — the clip is repeated
with ffmpeg, entirely in your browser, and cut to exactly the length you
asked for (default **30 seconds**). Prefer whole repetitions with no cut at
the end? Set **Target length** to `0` and **Or total plays** to how many
times the clip should play back-to-back. Because the audio is decoded and
re-encoded, the joins are sample-accurate — no container-splice clicks.

### Worked example

A 6-second rain recording that should run for a minute of background
ambience: upload `rain.mp3` and set **Target length** to `60` — the result
`rain-loop.mp3` plays the rain ten times over and stops at exactly 60 s. For
a meditation bell that must ring exactly three times, set **Target length**
to `0` and **Or total plays** to `3`.

### Duration or play count?

- **Target length (default)** — "make it N seconds long". The clip repeats as
  often as needed and the last repetition is cut mid-play if it doesn't fit.
  Right for background beds, ambience, hold music, sleep sounds.
- **Total plays** — "play it N times". The output length is a multiple of the
  clip's length, never cut mid-play. Right for chimes, counts, drills,
  practice phrases.

### Limits and edge cases

- Input files up to 10 MiB, output up to 10 MiB — at 192 kbps mp3 that's
  roughly 7 minutes of output, so very long targets need a modest bitrate
  format like mp3 rather than wav.
- Target length can be up to 3600 s; plays 2-100. Length `0` with no play
  count is rejected — there'd be nothing to do.
- A target shorter than the clip simply trims it — nothing repeats.
- Seamlessness depends on the clip: if it starts or ends with a click, a
  volume step, or room tone that doesn't match, every join will carry it.
  Trim the clip cleanly first (trim-audio), or add a tiny fade at both ends
  (audio-fade) before looping.
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC).
  Embedded album art is dropped.

## FAQ

<details>
<summary>Will the loop be truly seamless?</summary>

The joins themselves are sample-accurate — the audio is decoded and repeated
at the waveform level, not spliced as files. Whether it *sounds* seamless
depends on the clip: a sound that ends the way it begins loops invisibly,
while a clip with a click or an abrupt cut at either edge repeats that flaw
at every join. Trim the clip cleanly or fade its edges slightly first.

</details>

<details>
<summary>What's the difference between target length and total plays?</summary>

Target length gives you an exact output duration and may cut the final
repetition short (fine for ambience). Total plays always finishes the last
repetition, so the output is a whole multiple of the clip's length (right
for bells or spoken phrases where a cut would be jarring).

</details>

<details>
<summary>Why is my long loop failing or capped?</summary>

Outputs are capped at 10 MiB. At 192 kbps mp3 that's about 7 minutes; wav
hits the cap after roughly one minute. For long background beds, keep mp3 or
ogg as the format, or loop a longer source clip fewer times.

</details>

<details>
<summary>Can I crossfade the repetitions into each other?</summary>

Not in this tool — crossfading loop joins needs the clip to overlap itself,
which is a different ffmpeg graph. If the joins are audible, the usual fix
is a cleaner trim of the source clip; a slight fade at both ends
(audio-fade) also softens joins at the cost of a small dip.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
