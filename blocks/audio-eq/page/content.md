## Equalize audio in your browser

Pick an audio file and set how much to boost or cut each band — the EQ is
applied with ffmpeg, entirely in your browser. **Bass** is a low shelf around
100 Hz (warmth, boominess), **Mid** a 1 kHz peaking band (vocal presence,
"boxiness"), and **Treble** a high shelf from about 3 kHz up (brightness,
hiss). Gains are in decibels from -20 to +20; a band left at 0 is untouched,
and at least one band must be set.

### Worked example

A phone-recorded interview that sounds dull and slightly boomy: upload
`interview.m4a`, set **Bass** to `-4` and **Treble** to `5`, leave **Mid** at
`0` — the result `interview-eq.mp3` has the low-end rumble tamed and the
voices clearly brighter. Too harsh? Drop treble to `3`. Voice still buried?
Add **Mid** `3` for presence.

### Which band does what

- **Bass (low shelf, ~100 Hz)** — `+` warms up thin recordings; `-` tames
  boomy rooms, wind rumble and mic handling noise.
- **Mid (peak, 1 kHz)** — `+` brings vocals forward; `-` fixes a "boxy" or
  honky character.
- **Treble (high shelf, ~3 kHz+)** — `+` adds clarity and air to dull audio;
  `-` softens hiss, sibilance and harshness.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works.
- Gains are capped at ±20 dB per band — bigger moves are almost never what a
  recording needs and mostly add distortion or noise.
- Large boosts can push loud audio into clipping; if the result crackles,
  cut the opposite bands instead of boosting (e.g. -6 bass instead of +6
  treble), or lower the overall volume first with audio-volume-adjust.
- An EQ shapes tone, it can't isolate or remove a voice/instrument, and
  boosting treble can't restore detail a low-bitrate encode threw away.
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC).
  Embedded album art is dropped.

## FAQ

<details>
<summary>What do bass, mid and treble actually change?</summary>

They're the three classic tone-control bands: bass shapes everything below
roughly 100 Hz (kick drums, rumble, warmth), mid works around 1 kHz where
voices live, and treble shapes the top from about 3 kHz (clarity, hiss,
cymbals). Positive dB values make that band louder, negative quieter, 0
leaves it alone.

</details>

<details>
<summary>How many dB should I move a band?</summary>

Start small: 3-6 dB is clearly audible, and most real-world fixes stay under
±8 dB. Cutting usually sounds more natural than boosting. The ±20 dB range
exists for rescue jobs, not everyday tweaks.

</details>

<details>
<summary>Why does my boosted audio sound distorted?</summary>

Boosting a band raises peaks — audio already near full scale then clips. Try
cutting the other bands instead of boosting one (the tonal balance change is
the same), or run the file through audio-volume-adjust with a few dB of cut
first, then equalize.

</details>

<details>
<summary>Can this remove background noise or vocals?</summary>

No — an equalizer changes the level of frequency bands, and noise or vocals
overlap the same bands as everything else. A treble cut can soften hiss and a
bass cut can tame rumble, but full removal needs dedicated denoise/source
separation tools, which this isn't.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
