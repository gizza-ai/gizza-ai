## Resample audio in your browser

Change an audio file's **sample rate** — the number of samples per second, in
hertz — to a target you choose. Upload a file, enter a rate like `44100` or
`48000`, and it's resampled with ffmpeg's high-quality windowed-sinc resampler
(the same `swresample` engine used by professional pipelines), complete with an
anti-alias low-pass when downsampling. Everything runs locally in your browser
tab via ffmpeg compiled to WebAssembly, so your audio is never uploaded to a
server. Anything ffmpeg can decode works as input: mp3, wav, flac, m4a/aac,
ogg, opus and more.

### Worked example

You recorded a voice memo at 48 kHz but your transcription tool wants 16 kHz
mono-friendly speech audio. Upload `memo.wav`, set **Target sample rate** to
`16000`, leave **Output format** at `wav`, and you get `memo-16000hz.wav` at
16 kHz — smaller and exactly the rate the downstream tool expects. Going the
other way (16 kHz → `48000`) changes the rate but cannot invent detail the
source never captured; upsampling only makes the file play at the new rate.

### Common sample rates

- **8000 / 16000 Hz** — telephony and speech; small files, ideal for
  transcription and voice models.
- **22050 / 32000 Hz** — legacy and broadcast intermediate rates.
- **44100 Hz** — CD quality; the safe default for music.
- **48000 Hz** — the standard for video, DAWs and most editing workflows.
- **88200 / 96000 / 192000 Hz** — high-resolution studio rates for mastering
  and archival.

Any integer from 3000 to 384000 Hz is accepted — the presets above are just the
common ones.

### Output formats

- **WAV** (default) — lossless 16-bit PCM; the cleanest way to store a resample.
- **FLAC** — lossless and compressed; smaller than WAV, still a perfect copy.
- **MP3** — lossy; small and playable everywhere, encoded at 192 kbps.
- **OGG** — lossy Vorbis; open format, good quality per byte, 192 kbps.
- **M4A** — lossy AAC in an mp4 container; small high-quality files, 192 kbps.

### Limits and edge cases

- Input files up to 10 MiB.
- Choose **WAV** or **FLAC** to keep the resample lossless; the lossy formats
  re-encode at a fixed 192 kbps (dedicated bitrate control lives in the separate
  audio-convert tool).
- Upsampling to a higher rate never adds fidelity the source lacks — it only
  changes how many samples per second the file stores.
- Embedded album art is dropped: cover images ride along as a video stream,
  which audio-only formats like wav can't carry.
- The output keeps the original filename with a rate suffix and the new
  extension (`song.wav` → `song-16000hz.flac`).

## FAQ

<details>
<summary>What's the difference between resampling and converting audio?</summary>

Resampling changes the **sample rate** (how many samples per second, in Hz) —
that's the whole point of this tool. Converting changes the **container and
codec** (mp3 → wav, say) while keeping the rate. Use this tool when a downstream
system demands a specific rate like 16 kHz or 48 kHz; use audio-convert when you
just need a different file format or bitrate.

</details>

<details>
<summary>Does upsampling to 96 kHz or 192 kHz improve the quality?</summary>

No. Upsampling recomputes the waveform at more samples per second but cannot
recover frequencies or detail the original recording never captured. It's useful
when a tool or device requires a higher rate, not as a way to "upgrade" audio.
Downsampling, by contrast, genuinely reduces size and is applied with a proper
anti-alias filter so it stays clean.

</details>

<details>
<summary>Which sample rate should I pick?</summary>

`44100` for music (CD quality), `48000` for anything going into video or a DAW,
and `16000` (or `8000`) for speech and transcription where small files matter.
When in doubt, match the rate your target system or file already uses so nothing
has to resample it again.

</details>

<details>
<summary>Which output format keeps the resample cleanest?</summary>

`wav` (the default) or `flac` — both are lossless, so the resampled audio is
stored exactly. Pick `mp3`, `ogg` or `m4a` only when you need a smaller file and
can accept a lossy 192 kbps re-encode on top of the resample.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file entirely inside the browser tab — the audio never leaves your device.

</details>
