## About this tool

Turn long, slow footage — a sunset, a build, a drive, a whiteboard session —
into a short, fast **timelapse**, without leaving your browser. Pick a video,
choose how many times faster it should play (say **10×**), set an output frame
rate, and the clip is compressed in time locally. The file never leaves your
device.

## How it works

The tool runs two ffmpeg filters compiled to WebAssembly. First
[`setpts`](https://ffmpeg.org/ffmpeg-filters.html#setpts-1) divides each frame's
timestamp by your speed factor (`setpts=PTS/10`), so a 60-second clip is
squeezed into 6 seconds. That squeeze crams every original frame into a tiny
window, so a second [`fps`](https://ffmpeg.org/ffmpeg-filters.html#fps-1) filter
then **drops the surplus frames** down to your chosen output rate — that
frame-drop is exactly what makes a timelapse look smooth and stay small instead
of a stuttering all-frames blur.

The sped-up video is re-encoded to **H.264** at `-crf 20` (visually
near-transparent quality), forced to `yuv420p` with `+faststart` so it plays and
scrubs everywhere. **Audio is always removed** — a 10×-fast soundtrack is just
noise, and dropping it keeps the output small. An `.mp4`, `.mov`, `.m4v`, or
`.mkv` keeps its container; other inputs (`.webm`, `.ogv`, `.avi`, …) come out as
**MP4**.

## Example

Upload a **2-minute** clip of clouds rolling past, set the speed to **20×** and
the output to **30 fps**, and you get a **6-second** timelapse of the same scene
— roughly `120s ÷ 20 = 6s` — silent, small, and ready to share.

## Notes

- **Output length ≈ input length ÷ speed.** A 10-minute clip at 20× is about 30
  seconds. Pick the speed by how short you want the result.
- **Audio is dropped by design.** If you want to change playback speed while
  keeping the audio in sync, use a speed tool instead — this tool is built for
  the silent, fast timelapse look.
- **Frame drop, not interpolation.** The output rate is hit by dropping frames,
  not by synthesising new in-between frames (no motion smoothing).
- **Private by design.** Everything runs in your browser. No upload, no server,
  no watermark.
- Speed is clamped to **2–300×**; output fps to **1–60**.

## FAQ

<details>
<summary>How do I decide what speed to use?</summary>

Work backwards from how long you want the result. Output length is roughly the
input length divided by the speed, so a **5-minute** clip at **10×** is about 30
seconds, and at **20×** about 15 seconds. For a very long recording (an hour or
more) pick a high factor like **60×** or beyond; the default is **10×**. Any
value from **2×** to **300×** is accepted and clamped into that range.

</details>

<details>
<summary>Why is there no sound on the result?</summary>

A timelapse plays many times faster than real time, so the original audio would
be an unintelligible high-speed rush. The tool removes the audio track on
purpose — this also keeps the output file small. If you actually want to speed
up a clip **and** keep its audio pitch-corrected, that's a speed change rather
than a timelapse.

</details>

<details>
<summary>What does the output fps control do?</summary>

After the clip is sped up, it's re-sampled to the frame rate you pick — **30**
(web/general), **24** (a cinematic film look), **25** (PAL), or up to **60** for
extra-smooth motion. Lowering it drops more frames (smaller file); the default is
**30**. It does not add new frames — raising it just repeats existing ones.

</details>

<details>
<summary>Will a timelapse look smooth or choppy?</summary>

Because frames are dropped rather than blended, very high speed factors on
already-jerky footage can look stepped. For the smoothest result, shoot or start
from steady, high-frame-rate footage and keep the output fps at 30 or higher.
This tool does not do motion interpolation, which is what would smooth out
extreme speed-ups.

</details>

<details>
<summary>Does the file get uploaded anywhere?</summary>

No. The whole re-encode runs on your own CPU via ffmpeg compiled to
WebAssembly, so the video stays on your device. That also means encoding time
grows with the clip's duration and resolution — a long 4K recording can take a
few minutes, while a short phone clip finishes in seconds.

</details>
