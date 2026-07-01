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

<details>
<summary>Which video formats can I mute, and what do I get back?</summary>

Anything ffmpeg can read — mp4, webm, mov and mkv are the common cases. The
output stays in the same container as the input (an mp4 in gives an mp4 out)
and is named after the original with a `-muted` suffix, e.g.
`holiday.mp4` → `holiday-muted.mp4`.

</details>

<details>
<summary>Is there a file size limit?</summary>

Yes — the input video can be up to 25 MB, and the muted output is capped at
25 MB too. Since muting only drops the audio track, the output is always a bit
smaller than the input, so in practice only the input limit matters.

</details>
