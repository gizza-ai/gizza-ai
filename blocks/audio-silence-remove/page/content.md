## Remove silence from audio in your browser

Pick a recording and the tool strips dead air — leading silence, long pauses in
the middle, and trailing silence — using ffmpeg's `silenceremove` filter,
entirely in your browser. A quarter second of every removed gap is kept, so
speech keeps its natural rhythm instead of sounding hard-cut.

### Worked example

Tighten up a podcast interview: upload `interview.mp3` and leave the defaults —
**Silence threshold** `-30` dB and **Min gap** `0.5` seconds. Every pause
longer than half a second collapses to a 0.25 s beat, and the result
`interview-nosilence.mp3` is often 20–40% shorter with no lost words. If a
quiet room tone is being kept as "speech", lower the threshold to `-40`; if
breaths are being cut, raise the minimum gap to `1`.

### How the two knobs interact

- **Silence threshold (dB)** — how quiet counts as silent. `-30` dB suits
  normal voice recordings; use `-40`…`-50` for quiet/noisy sources so hiss
  isn't kept as speech. Must be ≤ 0 dB.
- **Min gap (seconds)** — once the audio has started, only silences at least
  this long are trimmed; shorter pauses (breaths, beats between sentences) are
  left untouched. Leading silence before the first sound is always trimmed,
  whatever its length.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works.
- 0.25 s of each removed gap is kept (fixed) — output is tightened, not
  robotically gapless.
- Music with intentional quiet passages can get shortened — this tool is meant
  for speech; raise the minimum gap if you must run it on music.
- A recording with no gaps longer than the minimum comes back (re-encoded but)
  the same length.
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC).
  Embedded album art is dropped.

## FAQ

<details>
<summary>Why is there still a short pause at each cut?</summary>

That's deliberate: 0.25 s of every removed gap is kept so sentences don't
slam into each other. Fully gapless cuts sound robotic and make speech hard
to follow.

</details>

<details>
<summary>My quiet recording lost words — what should I change?</summary>

Lower the threshold (e.g. `-40` or `-50` dB). Your speech is quieter than the
default -30 dB cutoff, so parts of it are being classified as silence. If only
word endings clip, also raise the minimum gap so brief dips aren't trimmed.

</details>

<details>
<summary>Does this work for videos too?</summary>

Not this tool — cutting silence out of a video needs the audio and video
streams cut identically to stay in sync. Use the separate video-silence-cut
tool for that; this one is audio-only and faster because of it.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
