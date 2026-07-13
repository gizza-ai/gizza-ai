## About this tool

The **freeze-frame effect** pauses the action on a single frame: the video plays
normally, stops dead on the moment you choose, holds that frame as a still for a
set number of seconds, then carries on from the same point. It's the classic
"…and everything stopped" beat used in trailers, sports highlights, reaction
reels, and tutorials.

Pick the **freeze point** (a timestamp in seconds) and how long to **hold** it,
then run the tool. Everything happens locally in your browser with ffmpeg
compiled to WebAssembly — your video is never uploaded to a server.

### Worked example

Say you have a 6-second clip and want the action to stop at the 2-second mark and
hold for 3 seconds:

- **Freeze at (seconds):** `2`
- **Hold for (seconds):** `3`

The result is a **9-second** video: it plays `0 → 2s` as normal, freezes the
frame at 2s for 3 seconds, then plays the remaining `2 → 6s`. The output is always
exactly *hold* seconds longer than the source.

### How it works

Under the hood the clip is split at your timestamp. The first part plays up to the
freeze point and then clones that last frame for the hold duration (ffmpeg's
`tpad` filter, at the source frame rate — no frame-rate guessing). The second part
is the untouched tail. The two are joined back into a single H.264 video.

## FAQ

<details>
<summary>How long can the freeze last?</summary>

You can hold a frame for up to **60 seconds**. The output video ends up exactly
that many seconds longer than your source. If you leave the field at its default
it holds for 2 seconds.

</details>

<details>
<summary>What happens to the audio?</summary>

The output is **video-only** — audio is dropped. A freeze-frame is a visual
effect, and cleanly re-timing an audio track around an inserted still (silence
during the hold, then the rest shifted later) is out of scope here. If you need
the soundtrack, add it back with a separate audio tool after freezing.

</details>

<details>
<summary>Which video formats can I use?</summary>

MP4, MOV, WebM, and MKV all work as input. The video is re-encoded to **H.264**
(the split-and-join can't stream-copy). MP4, MOV, and MKV keep their container;
other inputs (such as WebM) come out as **MP4**.

</details>

<details>
<summary>What if my freeze time is past the end of the clip?</summary>

If the timestamp is beyond the video's length, the **last frame** is held instead
— you can't freeze a frame that doesn't exist. Set the freeze point to `0` to hold
the very first frame as an intro still.

</details>

<details>
<summary>Is there a file-size limit?</summary>

The page processes videos up to about **25 MB** entirely in your browser, so
short clips work best. Larger or longer videos are slow to encode in WebAssembly —
trim first if needed. Nothing is uploaded; all processing is local.

</details>
