## Compress audio in your browser

Pick an audio file and a target bitrate — it's re-encoded with ffmpeg,
entirely in your browser, and comes out smaller. The default **96 kbps MP3**
roughly halves a typical 192 kbps music file and cuts a 320 kbps one to less
than a third, while staying pleasant to listen to. For voice recordings,
podcasts and lectures you can go down to **64 kbps** (or even 32) with little
practical loss.

### Worked example

A 7 MB `lecture.m4a` phone recording that's too big to email: upload it, set
**Target bitrate** to `64` and leave **Output format** on `mp3` — the result
`lecture-compressed.mp3` lands around 2 MB, and speech at 64 kbps still sounds
clean. For a music file, stay at `96` or use `128` if you can hear artifacts.

### Picking a bitrate

- **32–48 kbps** — smallest files; fine for voice notes you just need to keep.
- **64 kbps** — the sweet spot for speech: podcasts, lectures, interviews.
- **96 kbps (default)** — casual music listening, big savings.
- **128–192 kbps** — music that should stay close to the original.

The bitrate must be between 32 and 320 kbps — values outside that range are
rejected rather than silently adjusted.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works (mp3, wav,
  m4a, ogg, flac, and more).
- Compression is **lossy** and works by throwing away detail — it can't be
  undone, so keep the original if it matters. Re-compressing an already
  low-bitrate file degrades it further without saving much.
- If the source is already at or below the target bitrate, the output won't
  get meaningfully smaller — pick a lower bitrate instead.
- Output formats are lossy only (mp3, ogg, m4a). Converting to lossless
  wav/flac never shrinks a file — use the audio-convert tool for that.
- Embedded album art is dropped; it's often a surprising chunk of small
  files' size.

## FAQ

<details>
<summary>How much smaller will my file get?</summary>

Compressed size is roughly `bitrate × duration`, regardless of the input
size: at 96 kbps, one minute of audio is about 0.7 MB. So a 192 kbps MP3
compressed to 96 kbps halves; a WAV shrinks by more than 90%. If the source
is already below the target bitrate, there's nothing left to save.

</details>

<details>
<summary>Which bitrate should I use for speech vs music?</summary>

Speech stays intelligible far lower than music: 64 kbps is plenty for
podcasts and lectures, and 32-48 kbps still works for voice notes. Music
starts sounding noticeably worse below 96 kbps mp3; use 128 kbps or more if
fidelity matters.

</details>

<details>
<summary>Why is my output not smaller?</summary>

The output size is set by the target bitrate, not the input's. If your file
is already encoded at or below the bitrate you picked (say a 64 kbps
audiobook "compressed" to 96 kbps), re-encoding can't shrink it — and may
even grow it slightly. Pick a bitrate clearly below the source's to see real
savings.

</details>

<details>
<summary>Does compressing reduce the audio quality?</summary>

Yes — lossy compression permanently discards the least audible detail, and
each re-encode discards a bit more. At 96 kbps and above most people don't
notice on casual listening; at very low bitrates artifacts (muffling,
swishing) become audible. Keep your original file if you might need full
quality later.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
