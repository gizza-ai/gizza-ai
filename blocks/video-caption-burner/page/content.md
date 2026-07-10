## About this tool

Burn an SRT or WebVTT subtitle track directly into a video so the captions are part of the pixels, not a separate sidecar file. That makes the result reliable on social feeds, messaging apps, muted autoplay, and players that do not load subtitle tracks.

Upload a video, paste the subtitle file contents, then choose the caption position, font size, text color, and optional background bar. Processing runs locally in your browser with ffmpeg; your source video is not uploaded to gizza.ai.

### Worked example

Paste a caption track like this:

```srt
1
00:00:00,000 --> 00:00:02,000
Welcome to the demo

2
00:00:02,000 --> 00:00:04,000
These captions are burned in
```

Then upload a clip and keep the default bottom position, 24 px white text, and semi-transparent black background. The output is a playable video with those two captions permanently visible at their timed windows.

### Limits and edge cases

- Accepts SRT and WebVTT timing lines. Inline styling tags such as `<i>` are stripped to plain text.
- Up to 200 KB of subtitle text and 400 cues are accepted to keep the browser ffmpeg filter graph bounded.
- Videos are limited by the browser and the shared gizza media cap; large server-style batch jobs are out of scope.
- Output is H.264 video. MP4, MOV, M4V, and MKV inputs keep their container when possible; other containers are returned as MP4.
- ASS/SSA styling, custom fonts, and per-word karaoke effects are not interpreted.

## FAQ

<details>
<summary>What does “burn subtitles into a video” mean?</summary>

It means the captions become part of every video frame. The result does not need a separate `.srt` file, so the text stays visible when you upload the clip to sites or apps that ignore subtitle tracks.

</details>

<details>
<summary>Can I use WebVTT as well as SRT?</summary>

Yes. Paste either SRT timing lines with commas, such as `00:00:01,000`, or WebVTT timing lines with periods, such as `00:00:01.000`. A `WEBVTT` header and cue settings are accepted.

</details>

<details>
<summary>Why are my italic or colored subtitle tags not preserved?</summary>

This tool intentionally renders a plain, consistent caption style across the whole video. Basic inline tags are stripped so they cannot break the ffmpeg filter graph; use the page controls for the global text color, size, position, and background bar.

</details>

<details>
<summary>Does the video leave my computer?</summary>

No. The page runs ffmpeg in your browser and returns a local data URL for the captioned result. The CLI version processes the file on your machine as well.

</details>
