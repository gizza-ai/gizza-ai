## Fix audio that's out of sync with the video

Pick a video, tell it how far to nudge the **audio** relative to the picture,
and get the same clip back with the two lined up. The picture is
**stream-copied** (not re-encoded), so it stays byte-for-byte identical and
processing is fast; only the audio track is re-encoded, because its timeline is
being rewritten. Everything runs in your browser; nothing is uploaded.

### Which way do I shift it?

Watch a moment where a mouth moves or something makes a sharp sound, then pick a
direction with the **Offset** value:

- **Sound arrives before the picture** (voices are ahead of the lips) → the
  audio is early, so **delay** it with a **positive** offset (e.g. `200`).
- **Sound arrives after the picture** (lips move, then you hear it) → the audio
  lags, so **advance** it with a **negative** offset (e.g. `-200`).

Set the amount in **milliseconds** (the default) or **seconds** with the
**Unit** control — `200` ms is the same as `0.2` s. The range is ±60000 ms
(±60 s); `0` is rejected because it wouldn't change anything.

**Worked example:** a Bluetooth-mic recording where the voice trails the lips by
about a fifth of a second. Load `talk.mp4`, set **Offset** to `-200` with
**Unit = Milliseconds**, and you get `talk-synced.mp4` — same video, audio
pulled 200 ms earlier so it matches the lips.

### How the shift is done

- A **positive** offset pads silence at the front of the audio (`adelay`), so
  everything you hear plays that much later.
- A **negative** offset trims that much off the head of the audio and resets its
  timestamps, so everything plays earlier.
- The video stream is always copied losslessly; the picture never moves or
  re-encodes.

### Notes and limits

- The output keeps the same container (mp4 → mp4, webm → webm). WebM audio is
  re-encoded to Opus, everything else to AAC.
- This applies **one constant offset** to the whole clip. If the drift grows
  over the length of the video (audio slowly sliding out), a fixed shift can't
  fix it — that needs time-stretching, a different tool.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

### FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.

</details>

<details>
<summary>Should I use a positive or a negative offset?</summary>

If the **sound comes before the picture** (you hear the word before the lips
move), the audio is early — **delay** it with a **positive** value. If the
**sound comes after the picture** (lips move, then you hear it), the audio
lags — **advance** it with a **negative** value. A few frames is often enough:
one frame at 25 fps is about `40` ms.

</details>

<details>
<summary>What's a typical offset to try?</summary>

Bluetooth headphones and speakers commonly add about `-150` to `-250` ms of
audio lag, so `-200` ms is a good first guess. For most other drift, start near
`100`–`200` ms in whichever direction lines the sound up, then fine-tune.

</details>

<details>
<summary>Will nudging the audio hurt the video quality?</summary>

No. Only the audio is changed; the **picture is stream-copied without
re-encoding**, so the video quality is identical to the original.

</details>

<details>
<summary>The sync drifts more and more over the clip — will this fix it?</summary>

No. This tool applies a single **constant** shift to the whole clip, which fixes
audio that's uniformly early or late. Drift that grows over time means the audio
plays at a slightly wrong speed and needs time-stretching instead — a constant
offset can't correct it.

</details>

<details>
<summary>Which video formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, mkv and webm are the common cases. The
output keeps the input's container and is named after the original with a
`-synced` suffix (e.g. `clip.mp4` → `clip-synced.mp4`). The input and output are
each capped at 25 MB.

</details>
