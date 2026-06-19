## Transcode a video in your browser

Pick a video, choose a target format (MP4 or WebM), and get a re-encoded copy
instantly. The transcode runs entirely in your browser with ffmpeg compiled to
WebAssembly — your video is never uploaded to a server.

### Formats

- **MP4** — H.264 video + AAC audio with `+faststart` for quick playback.
  The most widely supported container across devices and browsers.
- **WebM** — VP9 video + Opus audio. Open, royalty-free, and usually smaller
  than MP4 at a similar quality.

### Tips

- Leave quality blank to use the default (75). Quality 1-100 maps to ffmpeg's
  CRF scale — higher quality means a larger file.
- Works offline once the page has loaded.
