## About this tool

Change how many frames per second a video plays at — without leaving your
browser. Pick a video, type a target frame rate (say **30**), and the clip is
re-timed locally. The file never leaves your device.

## How it works

The tool runs ffmpeg's [`fps`](https://ffmpeg.org/ffmpeg-filters.html#fps-1)
video filter, compiled to WebAssembly. When you **lower** the frame rate (e.g.
60 → 30 fps) it drops the extra frames; when you **raise** it (e.g. 24 → 60 fps)
it duplicates frames to fill the gaps. Either way the clip's **duration stays
the same** — only the number of frames per second changes.

Because the frame timing changes, the video stream is re-encoded to **H.264**
at `-crf 20` (visually near-transparent quality). The audio track is left as-is
and copied across untouched. An `.mp4`, `.mov`, `.m4v`, or `.mkv` keeps its
container; other inputs (`.webm`, `.ogv`, `.avi`, …) come out as **MP4** (and
their audio is re-encoded to AAC), since those containers can't hold H.264/AAC.

## Example

Upload a 60 fps screen recording, set the target to **30**, and you get the
same clip at half the frame rate — smaller and smoother on players that
struggle with 60 fps, with identical length and audio.

## Notes

- **Duration is preserved.** This changes frames-per-second, not playback speed.
  To make a clip play faster/slower, use a speed tool instead.
- **Frame drop/duplication, not interpolation.** Raising the fps repeats
  existing frames; it does not synthesise new in-between frames (no motion
  interpolation).
- **Private by design.** Everything runs in your browser. No upload, no server.
- The target fps is clamped to the **1–240** range.

## FAQ

<details>
<summary>Does changing the frame rate make my video play faster or slower?</summary>

No. The clip keeps its original **duration**. Lowering the fps drops frames and
raising it duplicates frames, but a 10-second clip stays 10 seconds long. If you
want the video to actually play faster or slower, that's a speed change, not a
frame-rate change.

</details>

<details>
<summary>What fps should I pick?</summary>

Common targets are **30** (web/general), **24** (a cinematic "film look"),
**25** (PAL / European broadcast), and **60** (smooth motion / gaming). The
default is **30**. Any value from **1** to **240** is accepted and clamped into
that range.

</details>

<details>
<summary>Can I get smooth slow motion by raising the fps?</summary>

Not from this tool. Raising the frame rate **duplicates** existing frames rather
than generating new in-between ones, so it won't look smoother than the source —
true smoothness needs motion interpolation, which this doesn't do.

</details>

<details>
<summary>Do the codecs or file format change?</summary>

The video is always re-encoded to **H.264** (needed because the frame timing
changes). An `.mp4`, `.mov`, `.m4v`, or `.mkv` keeps its container and its audio
is copied through untouched; anything else (`.webm`, `.ogv`, `.avi`, …) is
converted to **MP4** with its audio re-encoded to AAC, because those containers
can't hold H.264/AAC.

</details>

<details>
<summary>Why does it take a while on longer clips?</summary>

The whole re-encode runs on your own CPU via ffmpeg compiled to WebAssembly —
that's what keeps the video on your device. Encoding time grows with duration
and resolution, so a long 4K clip can take several minutes where a short phone
clip finishes in seconds.

</details>
