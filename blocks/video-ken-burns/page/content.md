## Animate a photo with a Ken Burns move

Pick a single still image, choose the **motion**, how long the clip runs and the
output size, and get back a short MP4 that slowly pans and zooms across the
picture — the classic "Ken Burns" documentary look. Everything runs with ffmpeg
inside your browser tab: nothing is uploaded, and the tool is free.

Under the hood the photo is scaled to cover the output frame at a 2× supersample
(which keeps the movement smooth instead of stair-stepping), then ffmpeg's
`zoompan` filter walks a straight-line path across it — pushing in, pulling back,
or gliding sideways — and encodes the frames to H.264 (`libx264`, `yuv420p`,
`+faststart`) so the clip plays everywhere.

### Motion, length and size

- **Motion** — the move applied to the photo:
  - **Zoom in** (default) — starts on the whole photo and pushes slowly toward
    the center.
  - **Zoom out** — starts tight and pulls back to reveal the full photo.
  - **Pan left / right / up / down** — holds the zoom and glides the frame
    across the picture in that direction.
- **Duration (1–30 s)** — how long the clip runs. The default is **5 s**.
- **Zoom amount (1.0–2.0)** — how far the move travels. `1.3` (default) is a
  gentle, natural push; `1.6`+ is dramatic. **Panning needs a value above 1.0**
  so there is off-screen room to slide into — at exactly `1.0` a pan has nowhere
  to move.
- **Width / Height** — the output size in pixels (default **1280 × 720**).
  Odd numbers are snapped down to even (H.264 requires it). Use a tall size like
  **1080 × 1920** for vertical social stories.
- **Frames per second (1–60)** — smoothness of the motion, default **30**.

### Worked example

You have `beach.jpg`, a wide landscape photo. Load it, leave **Motion** on
**Zoom in**, set **Duration** to `7` and **Zoom amount** to `1.3`. You get
`beach-kenburns.mp4` — a 7-second 1280 × 720 clip that eases from the full frame
into a gentle close-up. Want the opposite reveal? Switch **Motion** to **Zoom
out**. Making a vertical reel? Set **Width** `1080` and **Height** `1920`. The
page URL
`/tools/video-ken-burns/?direction=zoom-out&duration=5&zoom=1.5&width=1280&height=720`
deep-links to a pre-filled zoom-out reveal.

### Notes and limits

- This animates **one** still image per clip — it is not a slideshow builder and
  does not stitch multiple photos or add music.
- The move is a single straight-line pan/zoom for the whole clip; there are no
  keyframes, easing curves or mid-clip direction changes.
- A pan at zoom `1.0` can't move (no off-screen room) — raise the zoom, or the
  clip will look static.
- The photo is resampled to your chosen size, so a small source stretched to a
  large output will look soft; start from an image at least as large as the
  output.
- Input is capped at 12 MB and the output clip at 32 MB; both are processed in
  your browser's memory.

### FAQ

<details>
<summary>Is my photo uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the image never leaves your device.
Nothing is uploaded and nothing is stored.

</details>

<details>
<summary>What is the "Ken Burns" effect?</summary>

It's the slow pan-and-zoom applied to still photos in documentaries — named
after filmmaker Ken Burns, who used it to bring life to archival stills. Instead
of a static picture, the camera appears to drift and push across it, so a single
photo fills several seconds of video.

</details>

<details>
<summary>Why does my pan look like it isn't moving?</summary>

A pan slides the frame within the zoomed-in picture, so it needs room to travel.
At a **Zoom amount** of exactly `1.0` there is no off-screen area to move into,
so the frame stays put. Raise the zoom to `1.2`–`1.5` and the pan will glide as
expected.

</details>

<details>
<summary>What size and length should I use?</summary>

**1280 × 720** at **5 seconds** is a good default for a web clip. For a vertical
phone story use **1080 × 1920**; for a square post use **1080 × 1080**. Keep the
duration between about 3 and 8 seconds — long enough to read as motion, short
enough to stay lively. The maximum is 30 seconds.

</details>

<details>
<summary>What image formats can I use?</summary>

Any still image ffmpeg can read — JPEG, PNG and WebP are the common cases. The
output is always an H.264 MP4, which plays in every browser and on phones. The
input file is capped at 12 MB.

</details>
