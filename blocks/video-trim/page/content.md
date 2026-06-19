## About this tool

Cut a clip out of a video without leaving your browser. Pick a video, set a
**start time** and a **duration** (both in seconds), and you get just that window
back — the file never leaves your device.

## How it works

The tool runs ffmpeg, compiled to WebAssembly, with a **stream-copy** trim
(`-c copy`): it seeks to the start time, keeps `duration` seconds, and writes an
**mp4** — without re-encoding. That makes it fast and lossless: the original
video and audio streams are copied through untouched.

## Notes

- **Stream-copy needs mp4-compatible streams.** Because nothing is re-encoded,
  the source must already use mp4-friendly codecs (typically H.264 video / AAC
  audio). If it doesn't, ffmpeg reports a clear error — re-encode first (for
  example with the video-compress tool) and trim the result.
- **Keyframe seeking.** The start time snaps to the nearest preceding keyframe,
  so the cut may begin slightly before the exact second you asked for. This is
  the trade-off for a fast, lossless copy.
- **Private by design.** Everything runs in your browser. No upload, no server.
- Works offline once the page has loaded.
