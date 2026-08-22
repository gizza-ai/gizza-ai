## Drop the frames that never changed

Screen recordings, slideshow exports, tutorial captures and animation renders
spend most of their length showing the *same picture*. A 60 fps capture of you
reading a page for five seconds is 300 frames of one image. Load the clip here
and every consecutive repeat is detected and removed — the file gets smaller and
the timeline stops being a wall of identical frames. Everything runs with ffmpeg
inside your browser tab: nothing is uploaded, and the tool is free.

Detection uses ffmpeg's `mpdecimate` filter. It splits each frame into 8×8
blocks and compares it with the last frame it kept: if no block changed more
than the `hi` threshold, and fewer than `frac` of the blocks changed more than
`lo`, the frame is a duplicate and goes.

### The controls

- **Sensitivity (1–100)** — how eagerly a frame counts as a duplicate. It scales
  the `hi`/`lo` thresholds around ffmpeg's own defaults, which sit at **50**
  (`hi=768`, `lo=320`).
  - **10–30** — only near-identical frames go. Safest for camera footage or
    anything with film grain you want to keep.
  - **50** (default) — the standard setting for screen and slideshow captures.
  - **70–100** — also removes frames that merely *look* the same: a blinking
    text cursor, dithering, video-compression shimmer. Great for slides, risky
    for footage with slow fades (a gradual fade can be swallowed).
- **Timing** — the dropped frames leave a hole in the timeline; this decides
  what happens to it.
  - **Keep timing** (default) — every remaining frame stays at its original
    timestamp, so the clip is exactly as long as before and plays identically.
    The result is a *variable frame rate* file.
  - **Constant frame rate** — the kept frames are re-held on an even grid, which
    is what video editors and NLEs prefer. The near-duplicates become *exact*
    repeats, which cost the encoder almost nothing, so the file still shrinks.
  - **Compact** — the gaps are closed as well, so a recording with long static
    stretches becomes a short clip of just the moments that changed. Audio is
    dropped in this mode, because a copied track would drift out of sync with a
    timeline that no longer matches it.
- **Max frame rate (optional)** — caps the rate *before* the duplicate scan, the
  classic "halve a 60 fps capture to 30" step. Capping first is deliberate: if
  your source is slower than the cap, the frames the cap inserts are removed
  again by the scan, so it can never invent frames.
- **Output format** — **Auto** keeps mp4/mov/m4v/mkv as-is and turns anything
  else (e.g. a WebM screen recording) into MP4; **MP4** forces H.264 + AAC;
  **WebM** forces VP9 + Opus.
- **Changed-area threshold (`frac`)** — advanced. The fraction of a frame's
  blocks that must change for the frame to be kept (default `0.33`). Lower it to
  ~`0.05` when a small moving region matters (a mouse cursor, a subtitle line);
  raise it when only big changes count.

### Worked example

`demo.mp4` is a 3-second, 10 fps screen capture: 30 frames, but only three
distinct pictures — one per second. Load it with the defaults
(sensitivity `50`, timing **Keep timing**) and you get `demo-dedup.mp4` with
**3 frames** instead of 30, still playing over the original three seconds. Switch
**Timing** to **Compact** and the same clip becomes 3 frames over 0.3 s — the
pauses are gone. The URL
`/tools/video-dedup-frames/?sensitivity=70&timing=compact` deep-links to that
second setup with the fields pre-filled.

### Notes and limits

- Only **consecutive** duplicates are removed. A frame that reappears later
  (a cut back to an earlier slide) is kept — this is a decimator, not a
  content-wide de-duplicator.
- Filtering forces a re-encode, so the output is not byte-identical to the
  source (H.264 `crf 20`, or VP9 `crf 32` for WebM). Audio is stream-copied
  untouched whenever the container allows it, re-encoded to AAC/Opus when the
  container changes, and dropped in **Compact** mode.
- With **Keep timing** the very last frame has no following frame to hold it, so
  the video stream can end up to one duplicate-run shorter than the source. If
  the clip has audio, the file's overall length is unchanged.
- Variable-frame-rate output (**Keep timing**) plays fine in browsers and
  players, but some older editors dislike it — use **Constant frame rate** for
  those.
- Very noisy sources (heavy grain, low-bitrate camera footage) have no exact
  duplicates at all; raise the sensitivity or accept that there is nothing to
  drop.
- Input and output are each capped at 25 MB, since the file is processed in your
  browser's memory.

### FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.
Nothing is uploaded and nothing is stored.

</details>

<details>
<summary>Why did my video get barely smaller?</summary>

Two common reasons. First, the source may have no real duplicates: camera
footage with grain changes slightly in every frame — raise the **Sensitivity**
to 70+ and re-run. Second, the file may be mostly *audio*: a 3-second clip whose
video shrinks from 30 frames to 3 barely changes size if an AAC track dominates
the bytes. Check the reported output size before and after.

</details>

<details>
<summary>What is the difference between Keep timing, Constant and Compact?</summary>

**Keep timing** removes the duplicates but leaves each surviving frame where it
was, so the clip runs for the same length with a variable frame rate. **Constant
frame rate** re-holds those frames on an even grid, which editors prefer — the
repeats come back as exact copies that compress to almost nothing. **Compact**
also removes the *time* the duplicates occupied, so the clip gets shorter and
plays back as a fast recap of everything that changed (audio is dropped).

</details>

<details>
<summary>Should I set a max frame rate as well?</summary>

Only if the source rate is higher than you need. A 60 fps screen capture of a
document rarely needs more than 30 fps, and capping first makes the duplicate
scan cheaper. The cap is applied before the scan, so it can never add frames to
the output — if your source is already slower, the cap does nothing.

</details>

<details>
<summary>What does the frac setting really control?</summary>

`frac` is mpdecimate's own threshold for *how much of the picture* has to move.
At the default `0.33`, roughly a third of the 8×8 blocks must change before a
frame is considered new. If your recording only changes in a small region — a
mouse pointer, a caption, a progress bar — that never reaches a third of the
frame, so those frames get dropped; lower `frac` to about `0.05` to keep them.

</details>

<details>
<summary>Which formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read: mp4, mov, m4v, mkv and webm are the common cases. With
**Auto**, mp4/mov/m4v/mkv keep their container and everything else comes out as
MP4; you can also force MP4 or WebM. The input and the output are each capped at
25 MB.

</details>

<details>
<summary>Can I get an animated GIF out?</summary>

Not here — GIF export needs its own palette pass to look right, so it lives in a
dedicated video-to-GIF tool. De-duplicate first, then convert the result: the
GIF ends up smaller because the repeated frames are already gone.

</details>
