## GIF to MP4 / WebM

Animated GIFs are huge. Convert one to an **MP4** (H.264) or **WebM** (VP9) video
and it typically shrinks by 5-10x while looking the same. Everything runs in your
browser with ffmpeg; the GIF is never uploaded.

### How it works

- Pick a format: **mp4** (most compatible, default) or **webm** (often even
  smaller).
- Every frame of the GIF is re-encoded to video; the animation is preserved.
- Frames are scaled to even dimensions (H.264 / yuv420p require it) using a
  high-quality Lanczos filter.

### Notes

- MP4/WebM don't store "loop forever" the way a GIF does — set your player or the
  HTML `<video loop>` attribute to loop it.
- Re-encoding runs locally, so a long GIF takes a little time.

### FAQ

<details>
<summary>Is my GIF uploaded?</summary>

No — ffmpeg runs in your browser tab; the file never
leaves your device.

</details>

<details>
<summary>Which format is smaller?</summary>

WebM (VP9) is usually smaller; MP4 (H.264) plays
everywhere. Try both.

</details>
