## Clean up a grainy, noisy video

Pick a video, choose **how hard to denoise** its picture and which denoiser to
use, and get the same clip back with the grain and sensor noise smoothed away.
Everything runs with ffmpeg inside your browser tab — nothing is uploaded, and
the tool is free.

Because a denoise filter rewrites every frame, the picture is re-encoded to
H.264 (`-crf 20`, visually near-transparent). The audio is left untouched
(stream-copied) whenever the container can hold H.264/AAC — mp4, mov, m4v and
mkv keep their container; other inputs (e.g. webm) are converted to MP4 and the
audio re-encoded to AAC.

### Denoiser and strength

- **Denoiser** — **hqdn3d** (the default) is a fast 3D denoiser that averages
  each pixel both across neighbouring pixels (spatial) *and* across time
  (temporal), so grain is smoothed while static detail is kept. **nlmeans**
  (non-local means) is a slower, spatial-only denoiser that finds similar
  patches across the frame; it keeps edges and texture crisper on heavy noise,
  at the cost of speed.
- **Strength (1–100)** controls how aggressively noise is removed:
  - **Low (10–25)** — light grain reduction that keeps the image natural.
  - **Medium (30–50)** — the general sweet spot for typical camera/sensor
    noise. The default of **30** lands here.
  - **High (60–100)** — heavy noise or high-ISO footage; push too far and fine
    detail can start to smear or look waxy.

Start moderate and raise it only until the grain is gone — over-denoising a
video (smeared texture, "plastic" faces) is the usual mistake.

### Worked example

You have `night-clip.mp4`, a hand-held phone clip shot in low light that's full
of colour speckle. Load it, leave **Denoiser** on **hqdn3d**, and set
**Strength** to `45`. You get `night-clip-denoised.mp4` — the same footage with
the speckle cleaned up and the picture re-encoded to H.264. If faces look a
little soft, drop the strength; if heavy grain remains and you'd rather keep
edges crisp, switch the **Denoiser** to **nlmeans** and try `40`.

On the command line the same job is `gizza video-denoise --method hqdn3d
--strength 45 night-clip.mp4`, and the page URL
`/tools/video-denoise/?method=hqdn3d&strength=45` deep-links to the same
pre-filled settings.

### Notes and limits

- This is a classic ffmpeg denoiser, **not an AI upscaler or restoration
  model** — it reduces grain/noise; it does not invent lost detail or sharpen.
- One setting is applied to the whole clip; there is no per-scene control.
- **nlmeans** is CPU-heavy in the browser — expect it to be markedly slower than
  hqdn3d, especially on longer or higher-resolution clips.
- The picture is re-encoded, so the output is not byte-for-byte identical to the
  source even at low strength. `-crf 20` keeps the loss visually negligible.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

### FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.
Nothing is uploaded and nothing is stored.

</details>

<details>
<summary>What's the difference between hqdn3d and nlmeans?</summary>

**hqdn3d** is a fast 3D denoiser that smooths across both neighbouring pixels
and successive frames (temporal + spatial) — a great general-purpose default.
**nlmeans** (non-local means) is spatial-only and much slower, but it matches
similar patches across the frame, so it holds onto edges and texture better on
heavy noise. If hqdn3d looks smeared, try nlmeans; if you want speed, stay on
hqdn3d.

</details>

<details>
<summary>What strength should I use?</summary>

Start around **30** (the default) and raise it only until the grain is gone.
High values (60+) remove more noise but can make the picture look soft or
"plastic", so increase gradually and stop when it looks clean.

</details>

<details>
<summary>Will denoising reduce the video quality?</summary>

The picture is re-encoded to H.264 at `-crf 20`, which is visually
near-transparent, so the quality loss from encoding is negligible. The bigger
factor is strength: over-denoising smears fine detail, so keep it as low as the
footage allows. Audio is stream-copied untouched (except when a webm input is
converted to MP4, where it's re-encoded to AAC).

</details>

<details>
<summary>Which formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, m4v, mkv and webm are the common cases.
mp4/mov/m4v/mkv keep their container; other inputs (e.g. webm) come out as MP4.
The input and output are each capped at 25 MB.

</details>
