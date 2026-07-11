## Extract the audio track from an MP4 as M4A

An `.mp4` video is a container: it often holds H.264/HEVC video plus an AAC or
ALAC audio track. This tool drops the picture and rewraps the first audio stream
into an `.m4a` file with a **lossless remux**:

`ffmpeg -i in.mp4 -vn -map 0:a:0 -c:a copy out.m4a`

`-vn` removes the video stream, `-map 0:a:0` selects the first audio track, and
`-c:a copy` copies the already-compressed audio packets into the M4A container
without running an encoder. There is no quality loss, no bitrate setting, and no
codec conversion. It is the right tool when the audio already fits in M4A and
you want it extracted quickly.

Everything runs locally with ffmpeg in your browser tab. Your video is not
uploaded.

### Worked example

You recorded `meeting.mp4` on a phone and only need the audio. Drop the video on
this page and download `meeting.m4a`. If the MP4 contains AAC audio, the output
contains the exact same AAC packets, just without the video track. Duration and
quality are preserved; only the container changes.

### Limits and edge cases

- Input and output are each capped at **10 MiB**.
- The tool extracts the **first** audio stream (`0:a:0`). It does not combine or
  select among multiple tracks.
- It does not transcode. If you need MP3/WAV or a different bitrate, use
  extract-audio-from-video or audio-convert.
- Videos with no audio track will fail with an ffmpeg "matches no streams" style
  error.

## FAQ

<details>
<summary>Does MP4 to M4A lose quality?</summary>

No. The audio stream is copied with `-c:a copy`; it is not decoded and encoded
again. The output quality is identical to the audio inside the source MP4.

</details>

<details>
<summary>Why is there no bitrate or quality setting?</summary>

Bitrate settings only apply when re-encoding. This tool is deliberately a
lossless remux, so it preserves the original audio packets. Use audio-convert if
you want to change codec or bitrate.

</details>

<details>
<summary>Which audio track is extracted?</summary>

The first audio stream is extracted (`-map 0:a:0`). If a video contains multiple
language tracks or commentary tracks, this tool does not currently expose a track
selector.

</details>

<details>
<summary>Is my video uploaded?</summary>

No. ffmpeg runs inside your browser through WebAssembly. The source video stays
on your device.

</details>
