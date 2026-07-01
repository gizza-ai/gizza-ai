## Loop a video in your browser

Pick a video or GIF and repeat it into one continuous file — a set number of
times, or looped to a target duration. The loop runs entirely in your browser
with ffmpeg compiled to WebAssembly; your file is never uploaded.

### Options

- **Repeat count** — total number of plays (e.g. `3` gives you the clip three
  times back-to-back). Range 1–100.
- **Loop to duration** — set a target length in seconds and the clip is looped
  to fill it (trimmed at the end). This **overrides** the repeat count when set.

### Notes

- The output is **stream-copied** (no re-encode), so it's fast and keeps the
  original quality and format (mp4/webm/gif…).
- Works offline once the page has loaded.

## FAQ

<details>
<summary>What's the difference between repeat count and loop-to-duration?</summary>

Repeat count is the **total number of plays**: `3` produces the clip three times
back-to-back. Loop-to-duration instead loops the clip indefinitely and trims the
output at your target length. If you set a duration greater than 0, it takes
precedence and the count field is ignored.

</details>

<details>
<summary>How long or how many loops can I make?</summary>

The repeat count accepts 1–100 total plays, and the duration mode accepts up to
3600 seconds (one hour of output). Values outside those ranges are rejected before
ffmpeg runs.

</details>

<details>
<summary>Does looping re-encode the video and hurt quality?</summary>

No. The tool uses ffmpeg's `-stream_loop` input option with `-c copy`, so the
frames are copied bit-for-bit into the output. That means zero quality loss, the
same container format as the input, and much faster processing than a re-encode.

</details>

<details>
<summary>In duration mode, what happens if the target isn't a multiple of the clip length?</summary>

The final repetition is cut mid-play so the output lands exactly on your target
duration. If you need only complete plays, use the repeat-count mode instead.

</details>
