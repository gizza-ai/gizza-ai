## About this tool

Videos often carry **fully-black frames at the very start or end** — a black leader before
a recording begins, a fade-to-black outro, or dead frames left by a screen recorder or
camera. They pad the runtime, throw off thumbnails and auto-play previews, and look sloppy
on a re-upload. This tool finds those black edges automatically and trims the clip down to
the real picture — in two passes, the same way video engineers do it by hand with ffmpeg:

1. **Detect** — ffmpeg's `blackdetect` filter scans **every frame** and logs each run of
   black it finds (`black_start` → `black_end`). We read that log to locate the leading
   black run (one that begins at the very start) and the trailing black run (one that ends
   at the very end).
2. **Trim** — the clip is cut to the non-black span and re-encoded with H.264 at CRF 18
   (visually lossless tier); audio is re-encoded to AAC so it stays in sync at the exact
   cut points.

Everything runs in your browser via WebAssembly ffmpeg — the file is never uploaded. If
there is no black at the ends you asked for, the tool tells you instead of re-encoding the
clip unchanged.

### Worked example

A **10-second** screen recording that opens with **1.5 s of black** (leader) and ends with
a **1.8 s fade to black**:

- the detect pass logs `black_start:0 black_end:1.5` and `black_start:8.2 black_end:10`;
- the leading run starts at 0, so the new start is **1.5 s**; the trailing run ends at 10,
  so the new end is **8.2 s**;
- the trim pass keeps **1.5 s – 8.2 s**, producing a **6.7-second** clip with the black
  intro and outro gone.

Leave the defaults (**pixel threshold 0.10**, **black-pixel ratio 0.98**, **min duration
0.10 s**) for normal black edges. Set **Trim which ends** to *Start only* or *End only* to
keep one side untouched. If your black is really dark grey (a heavily compressed fade),
raise the pixel threshold toward **0.15**; if a logo or timestamp sits over the black and
keeps it from being detected, lower the black-pixel ratio toward **0.90**.

## Limits and edge cases

- Input is capped at **25 MB** in chat/CLI; in the browser the practical limit is your
  device's memory.
- **Only the two edges are trimmed.** Black frames in the *middle* of the clip are kept —
  this tool never cuts the interior. (To remove every black segment, that's a different
  job.)
- A black run only counts as an *edge* when it starts within **0.25 s** of the beginning or
  ends within **0.25 s** of the end; a black run just inside the clip is treated as content.
- **min duration** is the shortest black run the detector reports. At the default 0.10 s a
  single black frame in a 24–30 fps clip may be too brief to catch — lower it (e.g. 0.04)
  to trim a one-frame flash.
- If the **entire clip** reads as black at your settings, the tool refuses to trim (nothing
  would remain) and suggests lowering the thresholds or raising the minimum duration.
- Trimming requires re-encoding (a stream copy can only cut on a keyframe), so the video
  track is always re-encoded; CRF 18 keeps that visually transparent. Output keeps the
  input container for MP4/MOV/M4V/MKV, otherwise it becomes MP4.

## FAQ

<details>
<summary>How does it know which frames are black?</summary>

It runs ffmpeg's `blackdetect` filter over the whole clip. A pixel counts as black when
its brightness is below the **pixel black threshold**, and a whole frame counts as black
when at least the **black-pixel ratio** of its pixels are black. Consecutive black frames
lasting at least the **minimum duration** are reported as a run with a start and end
timestamp, which is what the trim is based on.

</details>

<details>
<summary>Why did it say "no black frames to trim"?</summary>

No fully-black run was found at the end(s) you selected. Either the clip genuinely has no
black intro/outro, or the black isn't dark enough to register — heavily compressed fades
are often dark grey rather than true black. Raise the **pixel black threshold** (try 0.15,
then 0.20) or lower the **black-pixel ratio** and run again. The tool prefers telling you
over re-encoding a clip it wouldn't change.

</details>

<details>
<summary>Does it also remove black *bars* (letterbox / pillarbox)?</summary>

No. Black bars are a *spatial* border around every frame; this tool trims *temporal* black
frames at the start and end of the timeline. They're different problems — use an autocrop
tool for letterbox and pillarbox bars.

</details>

<details>
<summary>Can it cut black frames from the middle of the video?</summary>

No — by design it only trims the leading and trailing edges. Interior black (e.g. a
scene transition) is kept so the tool never silently drops content you meant to keep.
Removing every black segment and re-joining the rest is a separate operation.

</details>

<details>
<summary>Will trimming hurt quality or break audio sync?</summary>

The cut itself is frame-accurate. The video track is re-encoded with H.264 at **CRF 18**
(generally considered visually lossless) because a stream copy can only cut on a keyframe.
Audio is re-encoded to AAC and cut at the same timestamps, so it stays in sync with the
trimmed video.

</details>
