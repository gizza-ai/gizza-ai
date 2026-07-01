## Change video speed

Pick a video and a **speed factor** — values above 1 speed it up (2 = twice as
fast), values below 1 slow it down (0.5 = half speed). The audio is kept in sync.
Everything runs in your browser with ffmpeg; nothing is uploaded.

### How it works

- The video timestamps are scaled (`setpts`) and the audio tempo is matched
  (`atempo`), so picture and sound stay aligned.
- Output keeps the original container (mp4, webm, …), re-encoded locally.
- Supported range is **0.25x to 4x**.

### Notes

- Re-encoding a long video takes time (it all runs on your machine).
- Slowing a video down does not add new frames — it stretches the existing ones.

### FAQ

<details>
<summary>Is my video uploaded?</summary>

No — ffmpeg runs in your browser tab; the file never
leaves your device.

</details>

<details>
<summary>Will the audio drift?</summary>

No — the audio tempo is scaled by the same factor as
the video, so they stay in sync.

</details>
