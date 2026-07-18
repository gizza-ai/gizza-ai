## Change audio speed in your browser

Pick an audio file and a speed **factor**: values above `1` play it faster,
values below `1` play it slower, and the pitch stays exactly where it was. This
is a time stretch (also called a tempo or BPM change), the opposite of the
tape-speed effect — a slowed track keeps its key instead of dropping in pitch,
and a sped-up podcast keeps its normal voices instead of turning to chipmunks.
Use it to listen to podcasts and audiobooks at `1.5×`, slow a solo or a
language lesson down for transcription and practice, or nudge a track to a
target BPM. Drag the slider, type a decimal, or hit one of the preset chips.
Everything runs entirely in your browser with ffmpeg compiled to WebAssembly,
so your audio is never uploaded to a server.

### Worked example

Speed up a lecture recording to play half again as fast: upload the file, set
**Speed factor** to `1.5`, leave **Format** on `mp3`, and run. The result plays
33% shorter at the same pitch and downloads as the original name with a
`-time-stretched.mp3` suffix. Some more common factors:

- `2` — double speed (half the duration); a 60-minute podcast becomes 30
  minutes.
- `1.25` — a gentle podcast/audiobook speed-up that stays easy to follow.
- `0.75` — 25% slower, a common setting for transcribing speech or learning a
  fast musical passage.
- `0.5` — half speed (double the duration), slow enough to pick out every note
  in a solo.

### Factor, percentage, and BPM

The factor is a plain multiplier of playback speed, so it converts directly to
the other ways speed gets described:

- **Percentage** — multiply by 100. `1.5` is 150% speed, `0.75` is 75% speed.
  A "20% faster" request is `1.2`; "40% slower" is `0.6`.
- **BPM** — the factor is the ratio of the two tempos: `factor = target BPM ÷
  source BPM`. To take a 90 BPM loop to 120 BPM, set `120 / 90 ≈ 1.33`. To take
  a 128 BPM track down to 100 BPM, set `100 / 128 ≈ 0.78`.

### How it works

The tool feeds the audio through ffmpeg's `atempo` filter, a WSOLA time-stretch
that changes tempo while leaving pitch untouched. `atempo` accepts `0.5×` to
`2×` per pass, so factors outside that window are split into a chain whose
product is your factor (for example `4×` becomes `2× then 2×`, and `0.25×`
becomes `0.5× then 0.5×`). Any attached album-art image is dropped so
audio-only formats like WAV encode cleanly, and the result is re-encoded to the
format you pick.

### Formats

- **MP3** — lossy at 192 kbps; small and playable everywhere (the default).
- **WAV** — lossless 16-bit PCM; largest, ideal for further editing.
- **OGG** — lossy Vorbis at 192 kbps; open format, good quality per byte.
- **FLAC** — lossless and compressed; smaller than WAV, still a perfect copy.
- **M4A** — AAC in an mp4 container; good quality at small sizes.

### Limits and edge cases

- Input files up to 10 MiB; anything ffmpeg can decode works (mp3, wav, flac,
  m4a/aac, ogg, opus, and most video containers' audio tracks).
- The factor ranges from `0.25×` to `4×`. Extreme stretches sound increasingly
  processed — the WSOLA artifacts (a slight fluttery or metallic quality) grow
  as you move away from `1`; between about `0.7×` and `1.5×` they are hard to
  hear on most material.
- `1` is rejected as a no-op rather than silently re-encoding your file — use
  the audio-convert tool if you only want a format change.
- The output is always re-encoded (the filter chain requires it).
- This is a pure time stretch: it changes tempo only. To change the key without
  changing the speed, use the audio-pitch-shift tool; for the tape-speed effect
  where pitch and speed move together, use the change-speed tool.

## FAQ

<details>
<summary>Will speeding up or slowing down the audio change its pitch?</summary>

No — that is the point of this tool. The tempo changes by the factor you pick
while the pitch and musical key stay exactly the same, so a sped-up podcast
keeps normal-sounding voices and a slowed song keeps its key. If you instead
want the tape-speed effect where pitch and speed move together, use the
change-speed tool; to move only the pitch and leave the speed alone, use the
audio-pitch-shift tool.

</details>

<details>
<summary>How do I convert a percentage or BPM into a factor?</summary>

For a percentage, divide by 100: 150% speed is a factor of `1.5`, and 80% speed
is `0.8`. For BPM, divide the target tempo by the source tempo: to go from 90
BPM to 120 BPM, use `120 / 90 ≈ 1.33`; to go from 128 BPM to 100 BPM, use
`100 / 128 ≈ 0.78`. A factor of `1` means no change.

</details>

<details>
<summary>What speed should I use for podcasts or audiobooks?</summary>

Most listeners find `1.25×` to `1.5×` comfortable for speech — noticeably
faster while still easy to follow. `2×` (double speed) works once you are used
to it and halves the listening time. For transcription or learning, go the
other way: `0.75×` or `0.5×` slows speech and music down enough to catch every
word or note.

</details>

<details>
<summary>How far can I stretch the audio?</summary>

The factor ranges from `0.25×` (quarter speed, four times as long) to `4×`
(quadruple speed, a quarter as long). Larger stretches are done by chaining
ffmpeg's `atempo` filter, which keeps the pitch correct, but the further you
get from `1×` the more the time-stretch artifacts show. Moderate changes
(roughly `0.7×`–`1.5×`) sound the cleanest.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file locally in the browser tab — the audio never leaves your device.

</details>
