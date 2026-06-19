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
