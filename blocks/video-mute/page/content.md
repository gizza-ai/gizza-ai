## Mute a video

Pick a video and get back a **silent copy** — the audio track is removed. The
video itself is stream-copied (not re-encoded), so it's fast and the picture
quality is byte-for-byte the same. Everything runs in your browser; nothing is
uploaded.

### Notes

- Lossless and quick: only the audio is dropped, the video stream is copied.
- The output keeps the original container format (mp4, webm, …).

### FAQ

<details>
<summary>Is my video uploaded?</summary>

No — ffmpeg runs in your browser tab; the file never
leaves your device.

</details>

<details>
<summary>Will the quality change?</summary>

No — the video is copied without re-encoding; only
the audio is removed.

</details>
