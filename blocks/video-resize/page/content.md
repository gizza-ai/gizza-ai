## Resize a video

Pick a video and set a target **width** and/or **height**. Leave one blank and
the aspect ratio is preserved (the missing side is computed automatically, and
always to an even number so the encoder is happy). It re-encodes in your browser
with ffmpeg; nothing is uploaded.

### Notes

- Give both width and height to force an exact size, or just one to scale
  proportionally.
- The output is re-encoded as H.264: an mp4, mov, m4v, or mkv keeps its
  container; other inputs (webm, …) come out as MP4.
- Downscaling shrinks the file; upscaling won't add real detail.

### FAQ

<details>
<summary>Is my video uploaded?</summary>

No — ffmpeg runs in your browser tab; the file never
leaves your device.

</details>

<details>
<summary>Why did my height change when I only set width?</summary>

To keep the aspect ratio —
the other dimension is computed for you.

</details>

<details>
<summary>What codec and quality does the resized video use?</summary>

The video stream is re-encoded with H.264 (libx264) at CRF 23 with the `medium`
preset — a sensible quality/size balance that plays everywhere. An mp4, mov,
m4v, or mkv keeps its container; other inputs (webm, …) are converted to MP4,
since those containers can't hold H.264/AAC.

</details>

<details>
<summary>Why did my requested size get nudged by one pixel?</summary>

H.264 with the standard yuv420p pixel format requires **even** width and height.
When a dimension is computed automatically it is always rounded to an even
number, so a request like width 641 can come out as 640×360 rather than failing
in the encoder.

</details>
