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

### FAQ

<details>
<summary>What exactly does the quality slider control?</summary>

Quality 1-100 maps linearly onto ffmpeg's CRF scale (51 = worst, 0 = best): the default 75 corresponds to roughly CRF 13, and lower values give smaller files. It's a constant-quality setting, not a bitrate — complex footage produces bigger files at the same quality number.

</details>

<details>
<summary>How big a video can I transcode?</summary>

The input file and the transcoded output are each limited to **10 MiB**. For larger clips, trim or compress the video first — encoding runs in a single browser tab, so short clips are the intended use.

</details>

<details>
<summary>Which codecs are used for MP4 and WebM?</summary>

MP4 uses H.264 video + AAC audio with `+faststart` (so playback can begin before the file finishes downloading); WebM uses VP9 video + Opus audio in constant-quality mode. Those are the two targets — there's no AVI/MKV/HEVC output.

</details>

<details>
<summary>How is this different from the video-compress tool?</summary>

video-transcode changes the container and codecs to your chosen target (MP4 or WebM), while video-compress re-encodes but keeps the input's original container. Use this one when you need a specific format, e.g. WebM for the open web.

</details>
