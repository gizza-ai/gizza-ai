## Resize a video

Pick a video and set a target **width** and/or **height**. Leave one blank and
the aspect ratio is preserved (the missing side is computed automatically, and
always to an even number so the encoder is happy). It re-encodes in your browser
with ffmpeg; nothing is uploaded.

### Notes

- Give both width and height to force an exact size, or just one to scale
  proportionally.
- The output keeps the original container (mp4, webm, …), re-encoded as H.264.
- Downscaling shrinks the file; upscaling won't add real detail.

### FAQ

**Is my video uploaded?** No — ffmpeg runs in your browser tab; the file never
leaves your device.

**Why did my height change when I only set width?** To keep the aspect ratio —
the other dimension is computed for you.
