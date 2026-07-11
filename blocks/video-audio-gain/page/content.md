## Make a video louder or quieter

Pick a video, choose how much to change its **audio volume**, and get the same
clip back — louder or quieter. The picture is **stream-copied** (not
re-encoded), so it stays byte-for-byte identical and processing is fast; only
the audio track is re-encoded. Everything runs in your browser; nothing is
uploaded.

### How much is "a lot"?

You can set the change two ways with the **Unit** control:

- **Decibels (dB)** — the audio engineer's scale. `+6` roughly doubles the
  perceived loudness, `-6` halves it. `+10` is a strong boost; `-10` a strong
  cut. Range: −60 to +60 dB (0 does nothing).
- **Factor (×)** — a plain multiplier. `2` is twice as loud (the same as
  "200%"), `0.5` is half, `10` is a big 1000% boost. Range: greater than 0 up
  to 16 (1 does nothing).

**Worked example:** a screen recording where the narration is too quiet. Load
`recording.mp4`, set **Amount** to `6` with **Unit = Decibels**, leave
**Prevent clipping** on, and you get `recording-gain.mp4` — same video, audio
about twice as loud, with peaks held at 0 dBFS so the boost doesn't distort.

### Prevent clipping (limiter)

When you boost, samples that were already near the maximum can be pushed past
it and clip (harsh distortion). The **Prevent clipping** option adds a peak
limiter that holds the output at 0 dBFS, so boosts stay clean. Turn it off if
you want an exact linear gain (for example when measuring levels).

### Notes and limits

- The video stream is copied losslessly; the output keeps the same container
  (mp4 → mp4, webm → webm). WebM audio is re-encoded to Opus, everything else
  to AAC.
- One constant gain is applied to the whole clip — this isn't a timeline/volume
  automation editor.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

### FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.

</details>

<details>
<summary>Will boosting the volume hurt the video quality?</summary>

No. Only the audio is changed; the **picture is stream-copied without
re-encoding**, so the video quality is identical to the original.

</details>

<details>
<summary>Can I make the video quieter or completely silent?</summary>

Quieter, yes — use a negative dB value (e.g. `-10`) or a factor below `1`
(e.g. `0.5`). To remove the audio entirely, use the **Mute a Video** tool
instead, which drops the audio track completely.

</details>

<details>
<summary>What's the difference between decibels and a factor?</summary>

They describe the same change on different scales. A factor of `2` (200%) is
about `+6` dB; `0.5` (50%) is about `-6` dB. Decibels match how loudness is
perceived; a factor is a straight multiplier on the waveform. Pick whichever is
more natural for you.

</details>

<details>
<summary>Why does the boost sometimes sound "capped"?</summary>

That's the **peak limiter** doing its job — it holds the loudest peaks at
0 dBFS so a big boost doesn't clip and distort. If you'd rather have an exact
linear gain with no limiting, turn **Prevent clipping** off.

</details>

<details>
<summary>Which video formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, mkv and webm are the common cases. The
output keeps the input's container and is named after the original with a
`-gain` suffix (e.g. `clip.mp4` → `clip-gain.mp4`). The input and output are
each capped at 25 MB.

</details>
