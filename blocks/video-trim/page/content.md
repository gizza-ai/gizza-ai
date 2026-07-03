## About this tool

Cut a clip out of a video without leaving your browser. Pick a video, set a
**start time** and a **duration** (both in seconds), and you get just that window
back — the file never leaves your device.

## How it works

The tool runs ffmpeg, compiled to WebAssembly, with a **stream-copy** trim
(`-c copy`): it seeks to the start time, keeps `duration` seconds, and writes
the result in the **same container as your input** (an mp4 stays mp4, a webm
stays webm) — without re-encoding. That makes it fast and lossless: the
original video and audio streams are copied through untouched.

## Notes

- **Lossless — no re-encode.** The streams are copied byte-for-byte, so there's
  no quality loss and it's fast. Because the output keeps your input's
  container (mp4 → mp4, webm → webm, mov → mov, mkv → mkv), the copy is always
  valid — there's no codec conversion to fail on.
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

The output keeps your input's container, so a stream copy is normally always
valid — an mp4 stays mp4, a webm stays webm, and their streams copy through
untouched. A codec error only shows up for an unusual container the tool can't
keep: it falls back to mp4, and streams that aren't mp4-compatible can't be
copied into it. In that case re-encode or compress the clip first (the
video-compress tool), then trim the result.

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
