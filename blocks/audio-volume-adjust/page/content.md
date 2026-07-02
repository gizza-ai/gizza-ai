## Change audio volume in your browser

Pick an audio file and a gain amount — the volume is changed with ffmpeg,
entirely in your browser. Work in **decibels** (+6 dB is roughly twice as
loud-feeling, -6 dB cuts it back) or switch the unit to **factor** (2 doubles
the amplitude, 0.5 halves it). A peak limiter is on by default so big boosts
hit 0 dBFS and stop instead of clipping into distortion.

### Worked example

A phone recording that's way too quiet: upload `memo.m4a`, leave **Amount** at
`6` (dB) and **Prevent clipping** ticked — the result `memo-volume.mp3` is
clearly louder with no crackle, because the limiter caps any peak that the
boost would have pushed past full scale. Still too quiet? Run it again, or use
`12`. Need an exact halving instead? Set **Unit** to `factor` and **Amount**
to `0.5`.

### Decibels or factor?

- **db** (default) — perceptual and additive: +6 then +6 equals +12. The range
  is ±60 dB; 0 is rejected since it wouldn't change anything.
- **factor** — exact linear scaling of the waveform: 2 doubles every sample
  value, 0.5 halves it. Accepted range is (0, 16].

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works.
- With the limiter ON, a large boost converges on "as loud as possible
  without clipping" rather than the literal gain — that's the point. Disable
  it (untick) if you need mathematically exact gain and accept clipping risk.
- Boosting can't add detail a very quiet, noisy recording never captured —
  the noise floor rises with the signal. For consistent loudness across a
  whole file, the audio-normalize tool (LUFS) is usually the better pick.
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC).
  Embedded album art is dropped.

## FAQ

<details>
<summary>How many dB make it "twice as loud"?</summary>

Roughly +10 dB feels twice as loud to most listeners; +6 dB doubles the
signal's amplitude. Try +6 first — it's a clearly audible boost — and repeat
or use +12 if the source is very quiet.

</details>

<details>
<summary>Why does my boosted file top out instead of getting ever louder?</summary>

The clipping limiter caps peaks at 0 dBFS — digital audio physically can't go
above full scale without distortion. Once peaks touch the ceiling, extra gain
only raises the quiet parts. If everything already peaks near full scale,
consider the audio-normalize tool to raise average loudness instead.

</details>

<details>
<summary>Should I use this or audio-normalize?</summary>

Use this tool for a simple fixed change ("make it 6 dB louder"). Use
audio-normalize when you need a standard loudness target (Spotify -14 LUFS,
podcast -16) or consistent volume across episodes — it measures your audio
and applies exactly the gain needed.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
