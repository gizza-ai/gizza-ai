## About this tool

Animated GIFs waste a lot of space on frames that show **exactly the same
picture** as the frame before them. Screen recordings are the worst offenders —
nothing moves for a second and a half, and the GIF still stores forty-five full
frames of it. This tool finds those runs of near-identical frames, keeps one of
each, and re-encodes the GIF. The animation still plays for the same length of
time, and the file is smaller. Everything happens in your browser; the GIF is
never uploaded.

## How it works

The tool runs ffmpeg, compiled to WebAssembly, in two stages:

1. **Detect duplicates.** The `mpdecimate` filter compares every frame with the
   previous kept one in 8×8 pixel blocks and marks it as a duplicate when the
   difference stays under the thresholds set by your **similarity threshold**.
2. **Rebuild the GIF.** The surviving frames are written back out with a palette
   generated from those exact frames (`palettegen` → `paletteuse`), so
   re-encoding does not visibly degrade the colours the way a fixed default
   palette would.

The kept frames keep their **original timestamps** (variable frame rate), so a
frame that stood still for a second now simply holds for a second instead of
being stored thirty times.

## Choosing a threshold

- **98% (default)** — removes frames that are identical or differ only by
  dithering noise and compression shimmer. Safe for almost any GIF.
- **100%** — the most conservative setting: only essentially bit-identical
  frames go. Use it when a GIF has very subtle motion, like a slow fade.
- **93-97%** — more aggressive. Removes frames that merely *look* the same, such
  as a blinking text cursor or a single moving pixel. Good for screen captures.
- **Below ~90%** — expect real motion to disappear; the animation starts
  collapsing towards a still image. Useful only if that is what you want.

## Notes

- **Private by design.** The GIF is processed in the page, not on a server.
- Input and output are capped at **32 MB**.
- The output is a looping GIF (`loop=0`), the same convention chat apps and
  social sites expect.
- Only the **consecutive** duplicates go. Two identical frames with a different
  frame between them are both kept — that is real animation, not padding.

## FAQ

<details>
<summary>Will the GIF play faster after the duplicate frames are removed?</summary>

No. Each kept frame stays at its original timestamp, so the frame that used to
be followed by twenty copies of itself now just holds on screen for that same
stretch of time. The animation looks the same and lasts the same; only the
number of stored frames goes down.

</details>

<details>
<summary>How much smaller will my GIF get?</summary>

It depends entirely on how repetitive the source is. A screen recording or a
slideshow export with long static stretches can shrink dramatically; a busy
hand-drawn animation where every frame is genuinely different may barely change
at all — in that case the tool correctly decides there is nothing to remove.

</details>

<details>
<summary>What exactly does the similarity threshold measure?</summary>

It is the share of the maximum possible difference between two frames that still
counts as "the same picture". At **98** a frame is a duplicate when its 8×8
blocks differ from the previous kept frame by less than 2% of that maximum. The
value is converted into `mpdecimate`'s `hi` and `lo` thresholds, keeping ffmpeg's
own 12:5 ratio between them, with the block fraction at its default `0.33`.

</details>

<details>
<summary>Can I use this on a video instead of a GIF?</summary>

Not here — this page only accepts `.gif` files, because it rewrites GIF frame
timing and writes a GIF back out. The same duplicate-frame detection applied to
MP4/WebM/MOV footage belongs in a video tool, where the audio track and the
output codec also have to be handled.

</details>

<details>
<summary>Why does the GIF sometimes look slightly different afterwards?</summary>

Removing frames means re-encoding, and re-encoding a GIF means re-picking its
256-colour palette. The tool builds that palette from the surviving frames and
applies ordered (Bayer) dithering, which keeps gradients smooth — but on a GIF
that was already heavily quantized you may spot a small change in the dither
pattern. Raising the threshold to **100** minimises how many frames change.

</details>
