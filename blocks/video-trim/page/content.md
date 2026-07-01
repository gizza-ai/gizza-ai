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

## FAQ

<details>
<summary>Why doesn't my clip start at the exact second I typed?</summary>

The trim seeks before decoding (`-ss` ahead of `-i`) and copies the streams
without re-encoding, so playback can only begin at a keyframe. The cut snaps to
the nearest keyframe before your start time — a small drift is normal and is
the price of a fast, lossless trim. Frame-exact cutting would require
re-encoding the video.

</details>

<details>
<summary>The trim failed with a codec error — what's wrong?</summary>

The output container is always mp4 and nothing is re-encoded, so the source
streams must be mp4-compatible (typically H.264 video with AAC audio). A WebM
with VP9/Opus, for example, can't be stream-copied into mp4 — convert or
compress it first (the video-compress tool re-encodes), then trim the result.

</details>

<details>
<summary>Does trimming reduce the video quality?</summary>

No. Stream-copy passes the original video and audio bytes through untouched —
only the container timestamps change. What you can't do is pick a fractional
window smaller than the keyframe spacing, and the duration must be greater
than 0 seconds.

</details>

<details>
<summary>Is my video uploaded anywhere while it's being trimmed?</summary>

No — ffmpeg runs as WebAssembly inside the page, so the file is read and cut
entirely on your device, and the page even keeps working offline once loaded.

</details>
