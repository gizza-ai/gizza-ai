## About this tool

Make a video file smaller without leaving your browser. Pick a video, choose a
quality level, and it's re-encoded locally — the file never leaves your device.

## How it works

The tool runs a single-pass [CRF](https://trac.ffmpeg.org/wiki/Encode/H.264#crf)
("constant rate factor") re-encode with ffmpeg, compiled to WebAssembly. The
video is encoded to **H.264** with **AAC** audio, keeping the original container
format (mp4 stays mp4, webm stays webm, and so on).

CRF is a quality knob: **lower CRF means higher quality and a larger file;
higher CRF means a smaller file with more visible compression**. The default of
**28** is a good "noticeably smaller, still watchable" starting point. The value
is clamped to the **18–34** range.

## Notes

- **Quality, not a target size.** This is a single-pass encode, so you choose a
  quality level rather than an exact output size. Hitting a precise byte target
  (for example "under 10 MB") reliably needs a two-pass encode — a planned
  follow-up.
- **Private by design.** Everything runs in your browser. No upload, no server.
- Re-encoding an already-heavily-compressed clip may not shrink it much; raise
  the CRF for a smaller file.
