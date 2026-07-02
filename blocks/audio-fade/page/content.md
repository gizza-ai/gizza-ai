## Fade audio in and out, in your browser

Pick an audio file and how many seconds each fade should last — the fades are
applied with ffmpeg, entirely in your browser. The default **3 seconds** each
way gives a gentle, radio-style open and close; set either side to `0` to
fade only the other one. Fades fix abrupt starts, clipped endings, and the
click you often hear when a track was cut mid-waveform.

### Worked example

A clip trimmed out of a longer recording starts and stops dead: upload
`clip.mp3` and leave both fields at `3` — the result `clip-fade.mp3` opens
smoothly and dies away naturally. For a ringtone, try **Fade-in** `0.5` and
**Fade-out** `1`: quick enough to keep the hook, soft enough to lose the
click. For a podcast outro over music, a long **Fade-out** of `8`–`10` works
well.

### Picking fade lengths

- **0.3–1 s** — removes clicks and abrupt edges without being audible as a
  "fade"; right for ringtones and sound effects.
- **2–4 s** — the classic musical fade; the default 3 s sits here.
- **5–15 s** — long DJ-style outros, background-music beds under speech.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works.
- Each fade can be 0–30 seconds; values outside that range are rejected.
- Both fades at 0 is rejected — there'd be nothing to do.
- A fade longer than the track simply fades across the whole file; if
  fade-in and fade-out together exceed the length, they overlap and the
  middle never reaches full volume.
- The fade curve is ffmpeg's default (triangular/linear gain ramp).
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC).
  Embedded album art is dropped.

## FAQ

<details>
<summary>How long should a fade be?</summary>

Under a second reads as "no click", not as a fade — use 0.3-1 s to clean up
cut points. Musical fades usually sit at 2-4 s, and long outros at 5-15 s.
When unsure, start with the 3 s default and adjust by ear.

</details>

<details>
<summary>Can I fade only one end?</summary>

Yes — set the other side to 0. A fade-in of 0 with a fade-out of 6 leaves
the start untouched and gives a six-second ending; that's the usual shape
for trimmed podcast segments.

</details>

<details>
<summary>Why does my short clip never reach full volume?</summary>

If fade-in + fade-out is longer than the clip, the two ramps overlap: the
fade-out starts before the fade-in finishes, so the middle stays below full
level. Use shorter fades for short clips — for a 4-second clip, keep the two
lengths under 2 s each.

</details>

<details>
<summary>Does fading change the length or quality of my audio?</summary>

The length stays exactly the same — fading changes volume over time, it
doesn't trim anything (use trim-audio to cut). Like every edit here the file
is re-encoded, so pick wav or flac if you need a lossless result.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
