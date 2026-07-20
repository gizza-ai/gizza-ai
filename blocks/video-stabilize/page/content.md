## Smooth out camera shake

Pick a shaky handheld clip, choose **how much shake to compensate** and what to
do with the frame edges, and get the same clip back with the wobble smoothed
away. Everything runs with ffmpeg inside your browser tab — nothing is
uploaded, and the tool is free.

The stabilizer (ffmpeg's motion-detection `deshake` filter) measures the
dominant frame-to-frame movement and shifts each frame to cancel it, so
handheld jitter is smoothed while intentional pans are kept. Because every
frame is rewritten, the picture is re-encoded to H.264 (`-crf 20`, visually
near-transparent). The audio is left untouched (stream-copied) whenever the
container can hold H.264/AAC — mp4, mov, m4v and mkv keep their container;
other inputs (e.g. webm) are converted to MP4 and the audio re-encoded to AAC.

### Strength and edge handling

- **Strength (1–100)** sets the motion search radius — how big a shake the
  stabilizer can detect and cancel. It steps through the four radii the
  deshake filter accepts: 1–25 → 16 px, 26–50 → 32 px, 51–75 → 48 px,
  76–100 → 64 px.
  - **Subtle (10–20)** — minor jitter, tripod bumps, breathing wobble.
  - **Normal (25–40)** — typical handheld phone footage. The default of **25**
    matches ffmpeg's own deshake default.
  - **Strong (50–100)** — walking shots and action-cam shake; very high values
    can wobble on intentional camera moves, so raise it gradually.
- **Edges** — shifting a frame to cancel shake exposes a band at its edges;
  this picks what fills it:
  - **Mirror** (default) — reflects the picture into the band. No zoom, full
    resolution, usually invisible on busy footage.
  - **Crop & zoom** — zooms in slightly (about 3–10 %, growing with strength)
    and center-crops so the moving edges fall outside the visible frame — what
    most stabilizers do by default. Costs a little field of view.
  - **Blank** — black bands, useful to *see* how much correction is applied.
  - **Original** — keeps the unstabilized source pixels at the edges.

### Worked example

You have `walk.mp4`, a 1080p clip filmed while walking, and the horizon bounces
with every step. Load it, set **Strength** to `60` and **Edges** to **Crop &
zoom**. You get `walk-stabilized.mp4` — the same footage with the bounce
smoothed and a slight (~7 %) zoom hiding the shifting edges. If the result
still jumps, raise the strength; if straight lines near the frame edge look
smeared on a *mirror* run, switch to **Crop & zoom**. The page URL
`/tools/video-stabilize/?borders=crop&strength=60` deep-links to the same
pre-filled settings.

### Notes and limits

- This is single-pass motion-compensation stabilization — great for **mild to
  moderate** handheld shake. Severe GoPro-style shake or rolling-shutter
  "jello" needs a two-pass or gyro-based stabilizer, which no in-browser tool
  provides.
- One setting applies to the whole clip; there is no per-scene control and no
  timeline selection — trim first with a cut tool if you only need a segment.
- **Crop & zoom** trades a little field of view for clean edges; on very small
  videos (under ~300 px) strong shake can still leave slight edge artifacts.
- The picture is re-encoded, so the output is not byte-for-byte identical to
  the source. `-crf 20` keeps the loss visually negligible.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

### FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your
device. Nothing is uploaded and nothing is stored.

</details>

<details>
<summary>What strength should I use?</summary>

Start at the default **25** for ordinary handheld footage. Use **10–20** for
minor jitter, **30–40** for a normal phone walk-and-talk, and **50 +** only for
genuinely rough footage (running, action cams). Too high a strength can start
"correcting" intentional pans, which shows up as a slow wobble — if you see
that, lower it.

</details>

<details>
<summary>Why do the edges of my stabilized video look odd?</summary>

To cancel shake the stabilizer shifts each frame, which exposes a thin band at
the frame edges. **Mirror** (the default) fills it by reflecting the picture —
usually invisible, but straight lines right at the edge can look smeared.
Switch **Edges** to **Crop & zoom** to hide the band completely at the cost of
a slight zoom, or **Blank** if you'd rather see black bands.

</details>

<details>
<summary>Does Crop &amp; zoom change my video's resolution?</summary>

Only marginally — the picture is scaled up by roughly 3–10 % (more at higher
strengths), then center-cropped back to within a couple of pixels of the
original width and height (dimensions are kept even for H.264). You lose a
little field of view at the edges, not output size.

</details>

<details>
<summary>Can it fix very shaky GoPro or running footage?</summary>

It will help, but this is a single-pass motion-compensation filter — it works
best on mild to moderate shake. Severe action-cam shake and rolling-shutter
"jello" need two-pass (analyze-then-transform) or gyro-data stabilization,
which is desktop-software territory. For those clips, run **Strength 60–100**
with **Crop & zoom** and expect an improvement, not a gimbal.

</details>

<details>
<summary>Which formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, m4v, mkv and webm are the common cases.
mp4/mov/m4v/mkv keep their container; other inputs (e.g. webm) come out as
MP4. The input and output are each capped at 25 MB.

</details>
