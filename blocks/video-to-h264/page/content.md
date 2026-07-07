## Convert any video to universally-playable H.264 MP4

Some videos just won't play — a `.webm` that Safari refuses, a 10-bit HEVC clip
an old TV can't decode, a 4:2:2 export a phone chokes on, an `.mkv` a web page
won't accept. The fix is almost always the same: re-encode to the one video
form that decodes essentially **everywhere** — **H.264** video in an **MP4**
container, **8-bit 4:2:0 (`yuv420p`)** chroma, with **`+faststart`** and **AAC**
audio.

This tool always applies that exact recipe. Drop in any clip and you get a
normalized `.mp4` that plays in every modern browser, phone, TV, and editor.
Everything runs locally with ffmpeg compiled to WebAssembly — your file is never
uploaded to a server.

### What "make it play anywhere" actually means

- **H.264 (libx264)** — the most broadly hardware- and software-decoded video
  codec on earth.
- **`yuv420p`** — forces 8-bit 4:2:0 chroma. Many players can't decode 10-bit,
  4:2:2, or 4:4:4; this normalizes them down so the picture always shows.
- **`+faststart`** — moves the MP4 index to the front so the video starts
  playing before it fully downloads (progressive web playback).
- **AAC audio** — the universally-supported audio codec (silent inputs stay
  silent — no track is invented).

### Profile — the compatibility dial

- **High** *(default)* — best compression; plays on every browser and device
  made since ~2012. Use it unless you have a reason not to.
- **Main** — a touch more legacy reach, marginally larger files.
- **Baseline** — the widest possible reach: no B-frames, no CABAC, for genuinely
  old or embedded players. Largest file for the same quality; only reach for it
  when something refuses **High**.

### Worked example

Take a 1-second `clip.webm` (VP9 video + Opus audio) that won't play in Safari.
Leave **Profile** on **High** and **Quality** at **75**, drop the file in, and
out comes `clip.mp4`: same 64×64 frame and ~1s duration, but now H.264 High /
`yuv420p` with AAC audio — plays in Safari, on an iPhone, and in a plain
`<video>` tag. Pick the **Maximum compatibility** preset instead and the same
clip comes out as **Constrained Baseline** for old hardware.

### Limits

- Input and output are each capped at **10 MiB** — compress or trim longer clips
  first (see the video-compress and video-trim tools).
- Output is **always** H.264/MP4. To target WebM/VP9 instead use
  **video-transcode**; to shrink a file use **video-compress**; to change the
  frame size use **video-resize**.
- This tool **always re-encodes** (that's the point). If your clip is already
  H.264/HEVC in a `.mov` and you only want to change the container without
  re-encoding, use **mov-to-mp4** for a lossless remux.

## FAQ

<details>
<summary>How is this different from mov-to-mp4 or video-transcode?</summary>

**mov-to-mp4** stream-copies (`-c copy`) when it can — it keeps whatever codec
and pixel format the source already has, so it does *not* guarantee H.264 High
or `yuv420p`. **video-transcode** switches the container/codec (MP4 ⇆ WebM) and
never pins a profile or pixel format. This tool **always re-encodes** and
**always** forces H.264 + a chosen profile + `yuv420p` + `+faststart` — its
entire job is the "make it play anywhere" normalize, not container conversion or
size reduction.

</details>

<details>
<summary>Why does my video play now when it didn't before?</summary>

The most common culprits are a codec or pixel format the target can't decode: a
VP9/AV1 `.webm`, a 10-bit HEVC clip, or 4:2:2 / 4:4:4 chroma from a pro camera.
Re-encoding to H.264 with `-pix_fmt yuv420p` collapses all of those to the
8-bit 4:2:0 H.264 that virtually every decoder supports, so the picture shows up
instead of a black frame or an "unsupported format" error.

</details>

<details>
<summary>Which profile should I choose?</summary>

Leave it on **High** — it plays on every browser and device from roughly 2012
onward and compresses best. Only switch to **Baseline** if a specific old or
embedded player (a legacy set-top box, an ancient phone, some hardware decoders)
refuses the file; Baseline drops B-frames and CABAC for maximum reach at the
cost of a larger file. **Main** sits in between.

</details>

<details>
<summary>Does the quality slider make it lossless?</summary>

No — this tool always re-encodes, so there is no bit-for-bit lossless mode. The
slider maps 1–100 onto a practical slice of libx264's CRF scale (higher = better
quality and a larger file): **100** is visually lossless (CRF 18), the default
**75** is roughly CRF 24, and **1** is small and low quality (CRF 40). If you
need a true lossless container change and your source is already H.264/HEVC, use
**mov-to-mp4** in remux mode instead.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using ffmpeg compiled to
WebAssembly — the file never leaves your device, and the page works offline once
it has loaded.

</details>
