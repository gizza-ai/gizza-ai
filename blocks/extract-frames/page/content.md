## Turn a video into a contact sheet

Load a video and this tool samples frames from it and tiles them into a single
**contact sheet** — a grid of thumbnails you can scan at a glance to storyboard a
clip, pick a cover image, or review scene changes. Everything runs locally in
your browser with ffmpeg (WebAssembly); the video is never uploaded.

Choose **how** frames are sampled:

- **Every N seconds (interval)** — one frame every `value` seconds. Set the rate
  to `2` for a frame every 2 seconds. Best for an even storyboard of a clip.
- **N frames per second (fps)** — sample `value` frames each second. Set `1` for
  one per second, `0.5` for one every two seconds.
- **At scene changes** — keep frames where the picture changes a lot (a cut, a
  new shot). `value` is the sensitivity from 0–1; lower catches more changes
  (`0.3` is a good start). The opening frame is always included.

Then set the **grid** (columns × rows), the **thumbnail width**, a **background
color** for the gaps, and **PNG** or **JPG** output.

### Worked example

Upload a 12-second clip, pick **Every N seconds** with rate **2**, a **4 × 3**
grid, **240 px** thumbnails and a **white** background. The tool samples one
frame every 2 seconds (6 frames) and lays them into the top of a 4-column,
3-row sheet — roughly **968 × 372 px** — with the unused cells filled white.
Right-click the result to save it, or use the Download link.

## Limits and good to know

- The sheet holds **up to columns × rows thumbnails** — the first grid's worth of
  sampled frames. If a high fps or short interval produces more frames than the
  grid has cells, only the earliest ones are shown; raise the interval, lower the
  fps, or enlarge the grid to cover the whole clip.
- Grid is **1–8 columns and 1–8 rows** (up to 64 thumbnails); thumbnail width is
  **16–800 px**. Height follows the source aspect ratio.
- **Scene mode needs visible changes.** A perfectly static clip has no scene
  cuts, so you'll get just the opening frame (padded out to the grid). Use
  interval or fps for a static screen recording.
- ffmpeg runs in your browser, so very large videos take longer and use more
  memory — trim or downscale huge files first.

## FAQ

<details>
<summary>Can I download each frame as a separate image or a ZIP?</summary>

Not from this page — it produces one contact-sheet image (a grid of frames),
which the browser can render and download as a single file. To grab one exact
frame at a chosen timestamp as its own PNG, use the companion **Extract a Video
Frame** tool instead.

</details>

<details>
<summary>What's the difference between "interval" and "fps" mode?</summary>

They're two ways to say the same thing. **Interval** is seconds *between* frames
(a rate of 2 means one frame every 2 seconds). **fps** is frames *per* second (a
rate of 2 means two frames each second). Use whichever is easier to think in —
interval for slow storyboards, fps when you want several frames a second.

</details>

<details>
<summary>How do I capture the cuts / distinct shots in a video?</summary>

Pick **At scene changes** and set the sensitivity (`value`) between 0 and 1.
Lower values (around `0.2`–`0.3`) catch more, subtler changes; higher values keep
only big jumps. ffmpeg scores how different each frame is from the previous one
and keeps the ones above your threshold, plus the opening frame.

</details>

<details>
<summary>Which video formats work?</summary>

Anything ffmpeg can decode in the browser — MP4/H.264, WebM, MOV and more. The
output sheet is a **PNG** (lossless, best for screen recordings and text) or
**JPG** (smaller, good for photographic footage).

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The video is decoded and the sheet is built entirely in your browser with
WebAssembly ffmpeg. Nothing leaves your device, and there's no account or upload
step.

</details>
