## About this tool

Long takes, interview reels and screen recordings are usually a string of separate
shots stitched together. This tool finds where one shot ends and the next begins —
the *scene cuts* — and writes each shot out as its own clip, plus a CSV listing
every scene's start, end and length.

Detection is ffmpeg's `scene` filter: each frame gets a difference score against the
one before it, and anything above the **detection sensitivity** counts as a cut.
Lower catches soft or graded transitions (and, past a point, fast camera moves);
higher keeps only hard cuts. **Shortest scene** merges boundaries that land closer
together than that, so a single cut spread over two frames still produces one clip.

Everything runs locally: ffmpeg is compiled to WebAssembly and the video never
leaves the browser.

### Worked example

A 3-second test clip made of three 1-second solid-colour shots (red → green → blue),
with the defaults (sensitivity `0.3`, shortest scene `0.6`s, re-encode):

| scene | start | end | duration | file |
| ----- | ----- | --- | -------- | ---- |
| 1 | 0 | 1 | 1 | `demo-Scene-001.mp4` |
| 2 | 1 | 2 | 1 | `demo-Scene-002.mp4` |
| 3 | 2 | 3 | 1 | `demo-Scene-003.mp4` |

Three download links appear, one per clip, plus `scenes.csv` holding exactly the
table above:

```
scene,start_seconds,end_seconds,duration_seconds,filename
1,0,1,1,demo-Scene-001.mp4
2,1,2,1,demo-Scene-002.mp4
3,2,3,1,demo-Scene-003.mp4
```

Raise the sensitivity to `0.7` on the same clip and no boundary passes the cutoff —
the page says so instead of handing back one clip identical to the input.

### Cut modes

**Re-encode** (default) decodes and re-encodes every clip with H.264 + AAC in MP4,
so each clip starts on exactly the detected frame. **Stream copy** remuxes the
original packets: near-instant and pixel-for-pixel lossless, but a clip can only
start on a keyframe, so its start may sit up to one GOP (often 1–10 seconds) before
the real cut, and the source container is kept.

### Limits and edge cases

- **25 MB input** on the command line; in the browser the practical ceiling is
  whatever fits in memory — a minute or two of 720p is comfortable, a full episode
  is not.
- **200 clips maximum.** A very low sensitivity on noisy footage can flag hundreds
  of frames; past the cap the tool stops and asks you to raise the sensitivity.
- Every clip is a separate ffmpeg pass, so a 20-scene re-encode takes roughly 20×
  one encode. Stream copy is the fast path.
- **Fades and dissolves** spread the change over many frames, so no single frame
  scores high. Lower the sensitivity to catch them; you may need to nudge the
  boundaries by hand afterwards.
- Detection is **visual only** — a hard cut between two similar-looking shots
  (same set, same lighting) may score below any usable threshold.
- Videos with **no audio track** work fine; keeping audio is simply a no-op.

## FAQ

<details>
<summary>What sensitivity should I use?</summary>

Start at the default `0.3`. If shots are being missed, step down to `0.2` and then
`0.15`. If a single shot is being chopped into pieces — usually fast motion, a
camera flash or heavy compression noise — step up to `0.4`–`0.5`. Sensitivity is
ffmpeg's scene score, a 0–1 measure of how different a frame is from the one before
it, so the same number behaves consistently across clips of similar content.

</details>

<details>
<summary>Why did I get fewer clips than there are cuts?</summary>

Two settings merge boundaries. **Shortest scene** (default `0.6`s) drops any cut
that lands less than that after the previous one, which is what stops one hard cut
from producing two clips when the transition straddles two frames; it also folds a
too-short final scene back into the one before it. And a cut whose score sits below
the **detection sensitivity** is never reported at all. Lower both if you are
missing real cuts.

</details>

<details>
<summary>Why does stream copy give clips that start early?</summary>

Copying packets means never decoding them, and a video stream can only be cut at a
keyframe. The clip therefore starts at the last keyframe at or before the detected
cut — with a typical 2–10 second keyframe interval, that can be seconds of the
previous shot at the head of the clip. Re-encode mode decodes and re-encodes, so it
can start on any frame; use it whenever the exact boundary matters.

</details>

<details>
<summary>What is in scenes.csv, and can I use it elsewhere?</summary>

One row per scene with `scene`, `start_seconds`, `end_seconds`, `duration_seconds`
and `filename`. It opens directly in a spreadsheet and is easy to feed into an edit
list, a chapter file, or your own ffmpeg script if you would rather cut the clips
yourself with different settings.

</details>

<details>
<summary>Does anything get uploaded?</summary>

No. ffmpeg is compiled to WebAssembly and runs inside the page, so the video, the
detection pass and every clip stay on your machine. That also means the work is done
by your CPU — long videos take real time, and a very large file can exhaust the
browser tab's memory.

</details>
