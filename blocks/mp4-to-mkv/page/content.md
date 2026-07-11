## Convert MP4 to MKV in your browser

An MP4 and a Matroska `.mkv` are both just *containers* — boxes that hold
already-compressed video, audio and subtitle streams. Moving from one to the
other doesn't require touching the media inside. This tool does a **lossless
remux**: it runs `ffmpeg -i in.mp4 -map 0 -c copy out.mkv`, which stream-copies
**every** track from the MP4 straight into an MKV wrapper. Nothing is
re-encoded, so the picture and sound are bit-for-bit identical, and a short clip
converts in a fraction of a second.

`-map 0` selects every stream in the input — the video, *all* audio tracks,
subtitles and data — rather than just the default video plus first audio. `-c
copy` copies those packets across without invoking an encoder. Because MKV is a
superset container that accepts essentially every codec MP4 can carry (H.264,
HEVC, AV1, VP9, MPEG-4 video; AAC, AC-3, MP3 audio), the remux always succeeds —
there is no codec that "won't fit," so this tool has no re-encode fallback and
no settings to tune.

Everything runs locally with ffmpeg compiled to WebAssembly — your file is never
uploaded to a server, and the page keeps working offline once it has loaded.

### Why move to MKV?

MP4 handles multiple audio tracks and soft (selectable) subtitles poorly. MKV
was built for exactly that. Remuxing to MKV first — losslessly, keeping your
original streams untouched — gives you a container you can later add extra audio
tracks or subtitle tracks to. If you instead want to *re-encode* (change codec,
resolution or quality), that's a different job: use video-transcode or
video-compress.

### Worked example

Take a 6-second clip `holiday.mp4` holding an H.264 video stream and an AAC
audio stream. Drop it in. Out comes `holiday.mkv` with the exact same H.264 and
AAC streams copied across — identical dimensions, identical duration, no visible
or audible change — now in a Matroska container ready to accept extra subtitle
or audio tracks. The conversion finishes almost instantly because nothing is
re-compressed.

### Limits

- Input and output are each capped at **10 MiB** — trim or compress longer clips
  first.
- This is a container remux only. It never changes codec, resolution or
  quality; to do that, use video-transcode or video-compress.
- Output is always an `.mkv` (`video/x-matroska`). The remux keeps whatever
  codecs the MP4 already used — it doesn't convert them.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: site/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown renders. -->

<details>
<summary>Does converting MP4 to MKV lose quality?</summary>

No. This is a lossless container remux, not a re-encode. The exact video, audio
and subtitle streams are copied into the MKV wrapper untouched with `-c copy`,
so the result is bit-for-bit the same media, just in a different box. Only tools
that genuinely re-compress the picture (video-transcode, video-compress) change
quality.

</details>

<details>
<summary>Why is the conversion so fast?</summary>

Because nothing is re-encoded. Re-encoding video is the slow, expensive part; a
remux only rewrites the container metadata and copies the already-compressed
packets across with `-map 0 -c copy`, so a short clip converts in well under a
second.

</details>

<details>
<summary>Does it keep all my audio tracks and subtitles?</summary>

Yes. `-map 0` selects every stream in the MP4 — the video, all audio tracks,
subtitles and data — and copies them all into the MKV. Nothing is dropped.
Because MKV is a superset container, whatever the MP4 held fits, which is exactly
why the remux always succeeds.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using ffmpeg compiled to
WebAssembly — the file never leaves your device, and the page works offline once
it has loaded.

</details>
