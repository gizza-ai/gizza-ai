## About this tool

Black bars appear when a video's aspect ratio doesn't match the frame it was packaged
in: **letterboxing** (bars above and below, e.g. a widescreen film inside a 4:3 frame)
and **pillarboxing** (bars left and right, e.g. a phone clip inside a 16:9 frame). They
waste pixels and bitrate, look bad on re-uploads, and defeat auto-cropping thumbnails.

This tool removes them automatically, in two passes — the same technique video engineers
use with ffmpeg by hand:

1. **Detect** — ffmpeg's `cropdetect` filter scans **every frame** of the clip and keeps
   the union of all picture content it sees. Because the box only ever grows, a
   fade-from-black opening or a dark scene can't trick it into cropping real content.
2. **Crop** — the detected rectangle is applied with `crop=W:H:X:Y` and the video is
   re-encoded with H.264 at CRF 18 (visually lossless tier). Audio is stream-copied
   untouched when the container allows it.

Everything runs in your browser via WebAssembly ffmpeg — the file is never uploaded.

### Worked example

A **320×240** clip that's really a **320×180** widescreen picture with 30-pixel black
bars top and bottom (classic 16:9-inside-4:3 letterbox):

- the detect pass settles on `crop=320:180:0:30` — 320×180 of real picture, starting
  30 px down;
- the crop pass re-encodes just that rectangle;
- the result is a **320×180** video with the bars gone. If the clip had no bars at all,
  the tool says so instead of silently re-encoding it.

Leave the defaults (**threshold 24**, **snap 2**) for normal black bars. If your source
was heavily compressed and the "black" is really dark grey, raise the threshold to
around **48**. Pick **snap 16** if a downstream encoder or player prefers
macroblock-aligned dimensions (the crop may keep up to 15 extra bar pixels per axis).

## Limits and edge cases

- Input is capped at **25 MB** in chat/CLI; in the browser the practical limit is your
  device's memory.
- The crop rectangle is **one box for the whole clip** — content that changes aspect
  ratio mid-video (e.g. a trailer switching between scope and flat) gets the union box,
  keeping everything visible.
- **Threshold 0** is the strictest setting: typical "black" bars in limited-range video
  sit at luma 16, so threshold 0 usually reports *no bars detected* — that's the tool
  refusing to guess, not a bug.
- **Threshold 255** makes every pixel count as black; the tool reports that the whole
  frame reads as black and asks you to lower the threshold.
- Cropping requires re-encoding (an ffmpeg stream copy cannot crop), so the video track
  is always re-encoded; CRF 18 keeps that visually transparent.

## FAQ

<details>
<summary>How does it know where the bars are?</summary>

It runs ffmpeg's `cropdetect` filter over the whole clip. Each frame, the filter
measures the smallest rectangle that contains everything brighter than the black
threshold; with the running-maximum mode used here, those rectangles are merged into a
union box across all frames. The final box is the picture; everything outside it is
bars. The crop values are read from the detection log, then applied in a second pass.

</details>

<details>
<summary>Why did it say "no black bars detected"?</summary>

The detected picture box covered the entire frame. Either your video genuinely has no
bars, or the bars aren't dark enough to count as black at the current threshold —
heavily compressed sources often have noisy, dark-grey bars. Raise the **black
threshold** (try 48, then 64) and run again. The tool prefers telling you over
re-encoding a video it wouldn't change.

</details>

<details>
<summary>Does it lower my video quality?</summary>

The crop itself is pixel-exact, but the video track must be re-encoded (ffmpeg filters
can't run in stream-copy mode). It re-encodes with H.264 at **CRF 18**, which is
generally considered visually lossless. Audio is copied through untouched when the
input container is MP4, MOV, M4V or MKV; other containers (e.g. WebM) are rewritten to
MP4 with AAC audio.

</details>

<details>
<summary>Can it remove white or colored borders too?</summary>

No — detection is luma-based black detection (that's what `cropdetect` measures), which
covers the letterbox/pillarbox bars that video players and encoders add. For colored
borders you know the size of, use a manual crop tool and give it the exact rectangle.

</details>

<details>
<summary>What do the two settings actually do?</summary>

**Black threshold (0–255)** is how dark a pixel must be to count as bar: 24 (the ffmpeg
default) matches true black bars; higher values catch grey-ish bars but risk eating
into dark scenes. **Snap crop to multiple of** rounds the cropped width/height down to
a multiple of 2, 4, 8 or 16: 2 removes the most bar while keeping dimensions H.264
legal, 16 yields the most encoder-friendly dimensions at the cost of possibly keeping a
sliver of bar.

</details>
