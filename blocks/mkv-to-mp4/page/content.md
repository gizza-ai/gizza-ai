## Convert MKV to MP4 in your browser

A Matroska `.mkv` and an `.mp4` are both just *containers* — boxes that wrap
already-compressed video and audio streams. When your MKV already holds the
codecs MP4 uses (H.264 or HEVC video with AAC audio, which is what a huge share
of MKV rips and recordings carry), converting it is just a **remux**: the video
and audio packets are copied straight into an MP4 wrapper with no re-encoding.
That's lossless, changes nothing about the picture or sound, and finishes in a
fraction of a second.

Everything runs locally with ffmpeg compiled to WebAssembly — your file is never
uploaded to a server, and the page keeps working offline once it has loaded.

### Two modes

- **Remux (default)** — stream-copies the video + audio with `-c copy` and
  writes a web-friendly `+faststart` MP4. Lossless, near-instant, no quality
  change. Use this whenever the MKV holds H.264/HEVC video and AAC audio.
- **Re-encode to H.264/AAC** — MKV is a superset container, so it often carries
  codecs MP4 can't legally hold: **VP8/VP9/AV1** video and **FLAC/Vorbis/Opus**
  audio. For those, re-encoding always produces a valid MP4; it's slower and
  lossy, so the quality slider (1–100, default 75) trades size against fidelity.

Both modes keep the video and audio and drop MKV-only extras — soft subtitles
(SRT/ASS/PGS) and font attachments — that MP4 can't carry.

### Worked example

Take a 6-second clip `episode.mkv` holding an H.264 video stream and an AAC
audio stream. Leave the mode on **Remux** and drop the file in. Out comes
`episode.mp4` with the exact same H.264 and AAC streams — identical duration and
dimensions, no visible change — ready to play on a phone, TV or anything that
rejects `.mkv`.

If instead the MKV held VP9 video or FLAC audio, **Remux** would fail with a
codec error; switch to **Re-encode to H.264/AAC** and it converts (re-encoding
the picture to H.264 and the sound to AAC).

### Limits

- Input and output are each capped at **10 MiB** — trim or compress longer clips
  first.
- Remux mode requires MP4-legal codecs (H.264/HEVC + AAC); if it errors, use
  Re-encode mode.
- Soft subtitle and attachment tracks are dropped — MP4 can't carry most of
  them. The video and all audio are kept.
- Output is always an `.mp4` (H.264/HEVC video, AAC audio). To target WebM
  instead, use the video-transcode tool.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: site/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown renders. -->

<details>
<summary>Does converting MKV to MP4 lose quality?</summary>

In **Remux** mode, no — it's a lossless container change. The exact H.264/HEVC
video and AAC audio streams are copied into the MP4 wrapper untouched, so the
result is bit-for-bit the same media, just in a different box. Only **Re-encode**
mode is lossy, because it genuinely re-compresses the picture and sound.

</details>

<details>
<summary>Why is the conversion so fast?</summary>

Because in the default Remux mode nothing is re-encoded. Re-encoding video is
the expensive part; a remux only rewrites the container metadata and copies the
already-compressed packets across with `-c copy`, so a short clip converts in
well under a second.

</details>

<details>
<summary>My MKV won't remux — it errors. What now?</summary>

That means the MKV holds a codec MP4 can't carry — commonly **VP8/VP9/AV1**
video or **FLAC/Vorbis/Opus** audio. Switch the mode to **Re-encode to
H.264/AAC**, which always produces a valid MP4. It's slower and lossy, so raise
the quality slider if you want to preserve more detail.

</details>

<details>
<summary>What happens to subtitles and attachments in the MKV?</summary>

They're dropped. MKV can carry soft subtitles (SRT/ASS/PGS) and font
attachments, but MP4 can't legally hold most of them — leaving them in makes an
otherwise-convertible file error out. This tool keeps the video and every audio
track and discards those MKV-only extras. If you need burned-in subtitles, use
the video-caption-burner tool first.

</details>

<details>
<summary>What does the quality slider do, and when does it matter?</summary>

It only applies in **Re-encode** mode, where 1–100 maps onto a practical slice
of ffmpeg's libx264 CRF scale (higher = better quality and a larger file): 100
is visually lossless (CRF 18), the default 75 is roughly CRF 24, and 1 is small
and low quality (CRF 40). In **Remux** mode it's ignored entirely, because a
stream-copy never re-encodes.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using ffmpeg compiled to
WebAssembly — the file never leaves your device, and the page works offline once
it has loaded.

</details>
