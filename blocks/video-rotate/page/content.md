## Rotate / flip a video

Pick a video, then rotate it clockwise by **90, 180, or 270** degrees and/or
**flip** it horizontally or vertically. It re-encodes in your browser with
ffmpeg; nothing is uploaded.

### How it works

- **Rotate** turns the whole frame (90 = quarter-turn clockwise, 270 =
  counter-clockwise, 180 = upside-down).
- **Flip** mirrors the frame left↔right (horizontal) or top↔bottom (vertical).
- You can combine a rotation with a flip. The video is re-encoded as H.264; the
  audio is copied unchanged.

### Notes

- Great for fixing phone clips recorded sideways or upside-down.
- The output keeps the original container format (mp4, webm, …).

### FAQ

**Is my video uploaded?** No — ffmpeg runs in your browser tab; the file never
leaves your device.
