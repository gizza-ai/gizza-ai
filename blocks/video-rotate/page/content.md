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

<details>
<summary>Is my video uploaded?</summary>

No — ffmpeg runs in your browser tab; the file never
leaves your device.

</details>

<details>
<summary>Can I rotate by an angle like 45°?</summary>

No — only quarter-turns are supported: 0, 90, 180, or 270 degrees clockwise
(any other value is rejected). At least one of rotate or flip has to be
active, otherwise there's nothing to do.

</details>

<details>
<summary>Does rotating lose quality?</summary>

The video track is re-encoded with H.264 (CRF 23, medium preset), so there's
a small generational loss — usually invisible for phone clips. The audio
track is copied bit-for-bit, unchanged, and the output keeps your original
container/extension.

</details>

<details>
<summary>If I set both a rotation and a flip, which happens first?</summary>

The rotation is applied first, then the flip mirrors the already-rotated
frame. So 90° + vertical flip means "quarter-turn clockwise, then mirror
top↔bottom" — pick the combination with that order in mind.

</details>
