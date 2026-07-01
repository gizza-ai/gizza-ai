## Extract a frame from a video in your browser

Pick a video, type a timestamp in seconds, and grab the frame at that moment as
a PNG image. The extraction runs entirely in your browser with ffmpeg compiled
to WebAssembly — your video is never uploaded to a server.

### How it works

- ffmpeg seeks to the timestamp you give and writes exactly one frame.
- The frame is saved as a **PNG** (lossless), so it's ready to download or feed
  into an image tool like resize, crop, or convert.

### Tips

- Use `0` to grab the very first frame.
- Fractional seconds work too — e.g. `1.5` for one and a half seconds in.
- A timestamp past the end of the video yields the last available frame.
- Works offline once the page has loaded.

## FAQ

<details>
<summary>How do I set the exact moment to capture?</summary>

Type the **timestamp in seconds**. Whole numbers and fractions both work, so
`0` grabs the first frame and `12.5` grabs the frame at twelve and a half
seconds. The value must be finite and zero or greater; a negative or invalid
timestamp is reported as an error.

</details>

<details>
<summary>What image format is the extracted frame?</summary>

Always a **PNG** (lossless), regardless of the input container — so you can
download it or feed it straight into another tool like resize, crop or convert
without a generation-loss re-encode. Exactly one frame is written per run.

</details>

<details>
<summary>Which video formats can I load?</summary>

Whatever the bundled ffmpeg build can demux and decode — the common web
containers and codecs like MP4/H.264, WebM/VP9 and MOV. Because everything runs
in your browser, a very large file is limited mainly by your device's memory.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The extraction runs with ffmpeg compiled to WebAssembly, so the video is
processed entirely in your browser and never sent to a server — it even works
offline once the page has loaded.

</details>
