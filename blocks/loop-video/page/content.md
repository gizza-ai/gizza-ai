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
