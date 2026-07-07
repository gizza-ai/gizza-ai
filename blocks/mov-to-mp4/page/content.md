## Convert MOV to MP4 in your browser

A QuickTime `.mov` and an `.mp4` are the same underlying container format —
they usually differ only in the box on the outside. When your MOV already holds
the codecs MP4 uses (H.264 or HEVC video with AAC audio, which is what almost
every iPhone, DSLR and mirrorless camera records), converting it is just a
**remux**: the video and audio packets are copied straight into an MP4 wrapper
with no re-encoding. That's lossless, changes nothing about the picture or
sound, and finishes in a fraction of a second.

Everything runs locally with ffmpeg compiled to WebAssembly — your file is never
uploaded to a server.

### Two modes

- **Remux (default)** — stream-copies with `-c copy` and writes a
  web-friendly `+faststart` MP4. Lossless, near-instant, no quality change.
  Use this for normal phone and camera clips.
- **Re-encode to H.264/AAC** — for MOVs whose codecs MP4 can't legally hold,
  the most common being **Apple ProRes** (from ScreenFlow, Final Cut exports,
  and pro cameras). Re-encoding always produces a valid MP4; it's slower and
  lossy, so the quality slider (1–100, default 75) trades size against fidelity.

### Worked example

Take a 6-second iPhone clip `beach.mov` (H.264 video + AAC audio). Leave the
mode on **Remux** and drop the file in. Out comes `beach.mp4` with the exact
same H.264 video stream and AAC audio stream — identical duration and
dimensions, no visible change — ready to attach to an email or upload anywhere
that rejects `.mov`.

If instead you had a ProRes screen recording, **Remux** would fail with a codec
error; switch to **Re-encode to H.264/AAC** and it converts (re-encoding the
picture to H.264).

### Limits

- Input and output are each capped at **10 MiB** — trim or compress longer clips
  first.
- Remux mode requires MP4-legal codecs; if it errors, use Re-encode mode.
- Output is always an `.mp4` (H.264/HEVC video, AAC audio). To target WebM
  instead, use the video-transcode tool.

## FAQ

<details>
<summary>Does converting MOV to MP4 lose quality?</summary>

In **Remux** mode, no — it's a lossless container change. The exact H.264/HEVC
video and AAC audio streams are copied into the MP4 wrapper untouched, so the
result is bit-for-bit the same video, just in a different box. Only
**Re-encode** mode is lossy, because it genuinely re-compresses the picture.

</details>

<details>
<summary>Why is the conversion so fast?</summary>

Because in the default Remux mode nothing is re-encoded. Re-encoding video is
the expensive part; a remux only rewrites the container metadata and copies the
already-compressed packets across, so a short clip converts in well under a
second.

</details>

<details>
<summary>My MOV won't remux — it errors. What now?</summary>

That means the MOV holds a codec MP4 can't carry — most often **Apple ProRes**
(common in ScreenFlow recordings and Final Cut exports). Switch the mode to
**Re-encode to H.264/AAC**, which always produces a valid MP4. It's slower and
lossy, so raise the quality slider if you want to preserve more detail.

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
