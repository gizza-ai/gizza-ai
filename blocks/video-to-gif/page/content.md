## About this tool

Turn a video into a shareable animated GIF without leaving your browser. Pick a
video, choose the **section** you want (a start time and a duration), set the
**frame rate** and **width**, and you get a looping GIF back — the file never
leaves your device.

## How it works

The tool runs ffmpeg, compiled to WebAssembly, with a two-stage palette filter.
First it generates a colour **palette tuned to your exact clip**
(`palettegen`), then it applies that palette to the frames (`paletteuse` with
dithering). That produces a much cleaner GIF at a much smaller size than the
naive fixed-256-colour conversion most converters use.

## Tips for a good GIF

- **Keep it short.** GIFs grow fast — a few seconds is usually plenty. Use the
  **duration** field to trim the clip.
- **Lower the frame rate.** 12 fps (the default) looks smooth for most clips and
  is a fraction of the size of a 30 fps GIF. Drop it to 8–10 fps for big size
  savings.
- **Scale it down.** Set a **width** (height follows automatically to keep the
  aspect ratio) — 320–480 px is great for chat and social. Leave it at 0 to keep
  the source size.
- The GIF **loops forever** by default.

## Notes

- **Private by design.** Everything runs in your browser. No upload, no server.
- Works offline once the page has loaded.
- Very large or very long videos can be slow or memory-heavy in the browser —
  trim to the section you need and pick a sensible width.

## FAQ

<details>
<summary>Is there a size limit on the video?</summary>

Yes — the converter caps both the input video and the resulting GIF at
**25 MB**. If your source is bigger, trim it first or convert just a section
using **start** and **duration**; a shorter window is also far faster to
process in the browser.

</details>

<details>
<summary>How do I convert only part of the video?</summary>

Set **start** (seconds into the video) and **duration** (clip length in
seconds). Leaving duration at 0 converts from the start point to the end. The
seek happens on the input side (`-ss` before `-i`), so jumping deep into a
long video is fast — frames before the start point are never decoded.

</details>

<details>
<summary>What are the frame-rate and width limits?</summary>

Frame rate accepts up to **60 fps** (default 12), and width up to **4096 px**
(0 keeps the source size). Height is computed automatically to preserve the
aspect ratio and rounded to an even number, and the resize uses high-quality
Lanczos scaling — so you only ever pick the width.

</details>

<details>
<summary>Does the GIF loop, and can I stop it looping?</summary>

The output is written with `loop=0`, the GIF convention for "repeat forever",
which is what chat apps and social sites expect. There's no play-once option
here — if you need a non-looping animation, a short MP4/WebM is usually the
better format anyway.

</details>
