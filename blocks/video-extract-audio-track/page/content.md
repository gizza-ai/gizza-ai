## Extract a video’s audio track without re-encoding

Use this when you already like the audio codec inside a video and just want the soundtrack as a standalone file. The tool runs ffmpeg locally in your browser with `-vn -map 0:a:<track> -c:a copy`, so it drops the video stream and copies the selected audio packets into a new container without changing quality.

### What to choose

- **Container** — `mka` is the safest default because Matroska audio accepts almost any codec. Choose `m4a` for AAC or ALAC audio from MP4/MOV files, and `ogg` for Vorbis or Opus audio from WebM/OGG-style sources.
- **Audio track** — `0` extracts the first audio stream. Use `1`, `2`, and so on for alternate language, commentary, or described-audio tracks in multi-track files.

### Worked example

For a normal MP4 with AAC audio, choose **M4A** and keep **track `0`**. The generated ffmpeg plan is equivalent to:

`ffmpeg -i in.mp4 -vn -map 0:a:0 -c:a copy out.m4a`

That remuxes the AAC stream into an M4A file without transcoding. If you are not sure which codec the source has, start with **MKA**.

### Limits and edge cases

- Max input and output size are **16 MB**.
- This is a demux/remux tool, not a converter. It does not change codec, bitrate, loudness, sample rate, or channels.
- Container compatibility still matters: AAC usually belongs in M4A, Vorbis/Opus in OGG, and unusual codecs in MKA.
- If the selected track does not exist, ffmpeg returns an error. Try track `0` first, then `1` for the second audio stream.

<details>
<summary>Is this lossless?</summary>

Yes. The audio stream is copied with ffmpeg’s `-c:a copy`, so samples are not decoded and encoded again. The output quality is the same as the selected audio track in the source video.

</details>

<details>
<summary>Why does the default output use MKA?</summary>

MKA is the audio-only Matroska container and can hold many codecs. It is the safest default when you do not know whether the source audio is AAC, Opus, Vorbis, ALAC, or something else.

</details>

<details>
<summary>When should I choose M4A or OGG?</summary>

Choose M4A for AAC or ALAC audio, especially from MP4 and MOV files. Choose OGG for Vorbis or Opus audio, commonly found in WebM-style workflows. If you pick a container that cannot hold the source codec, use MKA instead.

</details>

<details>
<summary>How is this different from extracting MP3 or WAV?</summary>

MP3 and WAV extraction requires re-encoding or decoding the audio into a new format. This tool keeps the original codec untouched; use the separate extract-audio-from-video or audio-convert tools when you want conversion instead of lossless demuxing.

</details>
