## Shift audio pitch in your browser

Pick an audio file and how many semitones to move it: positive values raise
the pitch, negative values lower it, and the speed stays exactly the same. Use
it to transpose a backing track into a singable key, deepen a voice recording,
or make the classic chipmunk / slowed-and-deep effects without touching the
tempo. The shift runs entirely in your browser with ffmpeg compiled to
WebAssembly, so your audio is never uploaded to a server.

### Worked example

Transpose a backing track up a whole step (D to E): upload the track, set
**Semitones** to `2`, leave **Format** on `mp3`, and run. The result plays in
the new key at the original tempo and downloads as the original name with a
`-pitch-shifted.mp3` suffix. Some more common shifts:

- `12` — one octave up (a 440 Hz A becomes an 880 Hz A).
- `-12` — one octave down.
- `-3` — lower a song three semitones into a more comfortable vocal range.
- `0.5` — a quarter-tone nudge (50 cents) for tuning mismatches, e.g. matching
  an A=432 Hz recording to A=440 Hz is about `0.32` semitones.

### How it works

The tool uses the classic resample method, fully offline: the audio is
resampled to a new rate (which shifts pitch and speed together, like changing
tape speed), then time-stretched back to the original duration with ffmpeg's
`atempo` filter. Pitch accuracy is within a fraction of a cent, and the output
duration matches the input.

### Formats

- **MP3** — lossy at 192 kbps; small and playable everywhere (the default).
- **WAV** — lossless 16-bit PCM; largest, ideal for further editing.
- **OGG** — lossy Vorbis at 192 kbps; open format, good quality per byte.
- **FLAC** — lossless and compressed; smaller than WAV, still a perfect copy.
- **M4A** — AAC in an mp4 container; good quality at small sizes.

### Limits and edge cases

- Input files up to 10 MiB; anything ffmpeg can decode works (mp3, wav, flac,
  m4a/aac, ogg, opus, and most video containers' audio tracks).
- Shifts are limited to ±24 semitones (two octaves). Large shifts sound
  increasingly processed — time-stretching artifacts (a slight metallic or
  fluttery quality) grow with the shift size; within ±5 semitones they are
  hard to hear on most material.
- `0` semitones is rejected as a no-op rather than silently re-encoding your
  file — use the audio-convert tool if you only want a format change.
- The output is always re-encoded (the filter chain requires it) and resampled
  to 44.1 kHz, the CD-standard rate most music already uses.
- Everything in the mix shifts together — this transposes the whole track, it
  cannot re-tune one instrument or correct a single off-key note.

## FAQ

<details>
<summary>Will changing the pitch also change the speed of my audio?</summary>

No — that is the point of this tool. The pitch moves by the semitones you
choose while the tempo and total duration stay the same. If you want the
tape-speed effect where pitch and speed change together (or a speed change
with the pitch preserved), use the change-speed tool instead.

</details>

<details>
<summary>How many semitones do I need to change key?</summary>

Count the steps between the keys: each semitone is one step on a piano
(including black keys). C to D is `2`, C to E is `4`, down a fourth is `-5`.
An octave is `12`. Fractional values work too — `0.5` is a quarter tone, and
one cent is `0.01`, so you can fix small tuning offsets precisely.

</details>

<details>
<summary>Can I shift a voice without it sounding robotic?</summary>

Small shifts (roughly ±4 semitones) keep voices natural-sounding. Bigger
shifts change the character noticeably — up sounds chipmunk-like, down sounds
giant-like — because the formants (the resonances that make a voice sound
human) shift along with the pitch. That's fun for effects, but there's no
formant-preserving mode in this tool.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
