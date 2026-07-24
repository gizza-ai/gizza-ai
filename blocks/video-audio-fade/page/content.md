## Fade the sound in or out without changing the picture

Use this tool when a video's **audio** needs a softer entrance, a cleaner ending,
or both. Choose a fade-in length, a fade-out length, or set one side to `0` to
skip it. The video stream is **copied without re-encoding**, so the picture stays
untouched; only the audio is re-encoded because its volume curve changes.
Everything runs in your browser and nothing is uploaded.

**Worked example:** load `intro.mp4`, set **Fade in** to `3` seconds and **Fade
out** to `4` seconds. The result starts from silence, reaches full volume over
three seconds, keeps the picture unchanged, and fades the sound down over the
last four seconds.

### How the fade is applied

- **Fade in** ramps audio from silence to full volume starting at the beginning.
- **Fade out** ramps audio from full volume to silence at the end. Internally it
  uses a reverse/fade/reverse ffmpeg chain, so it does not need to know the
  video's duration before processing.
- A value of `0` skips that side; both values at `0` are rejected because the
  output would be unchanged.

### Notes and limits

- Each fade can be from `0` to `30` seconds, in half-second steps on the page.
- The output keeps the same container where possible (`mp4` → `mp4`, `webm` →
  `webm`). WebM audio is encoded as Opus; other common video containers use AAC.
- This is audio-only. It does not fade the picture to black or crossfade between
  clips; use a video-picture fade or editor for that.
- Input and output are each capped at 25 MB because ffmpeg runs in the browser.

### FAQ

<details>
<summary>Will this change the video quality?</summary>

No. The picture stream is copied without re-encoding, so the visual quality is
preserved. Only the audio track is decoded and re-encoded to apply the fade.

</details>

<details>
<summary>Can I fade only the ending?</summary>

Yes. Set **Fade in** to `0` and set **Fade out** to the number of seconds you
want. You can also do the opposite for a fade-in-only result.

</details>

<details>
<summary>Why is fade-out allowed without entering the video duration?</summary>

The ffmpeg plan reverses the audio, applies a fade-in to that reversed stream,
and reverses it back. That creates an end fade without needing to calculate
`duration - fade_length` in advance.

</details>

<details>
<summary>What happens if both fade lengths are 0?</summary>

The tool rejects that as a no-op. Set at least one fade length above zero so the
processed video is meaningfully different from the input.

</details>

<details>
<summary>Can this fade the image to black too?</summary>

No. This tool deliberately changes only the audio while copying the picture. A
picture fade or timeline crossfade is a different video-editing operation.

</details>
