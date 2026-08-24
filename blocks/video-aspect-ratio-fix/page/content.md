## About this tool

Some videos play **stretched, squashed, or anamorphic**: circles look like ovals,
faces look narrow or fat, or a widescreen recording shows up letterboxed inside a
4:3 box. Almost always the encoded pixels are perfectly fine — the container is
simply telling the player the wrong shape. Cameras that record a squeezed
("anamorphic") frame, DVD/DV rips, screen recorders, and badly configured
transcoders all produce files like this.

This tool rewrites the **display aspect ratio (DAR) tag** in the container with
ffmpeg stream copy (`-map 0 -c copy -aspect W:H`). The file is remuxed with a
fresh header carrying the correct ratio, and every audio and video packet is
copied bit-for-bit — nothing is decoded, nothing is re-encoded, and the output is
normally the same size as the input, byte for byte. Everything runs locally in
your browser; the file is never uploaded.

Three terms are worth knowing:

- **DAR** — display aspect ratio: the shape you actually see, e.g. 16:9.
- **SAR/PAR** — sample (pixel) aspect ratio: the shape of a single pixel. Square
  pixels are 1:1.
- The stored frame size in pixels. `DAR = frame width ÷ frame height × SAR`.

Because this tool only sets DAR, ffmpeg derives the new SAR for you and the
stored frame size never changes.

## Worked example

A camera records a 16:9 scene into a 640×480 frame and forgets to flag it, so it
plays squashed. Upload `squeezed.mp4`, leave **Display aspect ratio** on `16:9`,
keep **Output container** on `keep`, and download `squeezed-aspect-fixed.mp4`.

`ffprobe` before and after:

```
before:  width=640  height=480  sample_aspect_ratio=1:1  display_aspect_ratio=4:3
after:   width=640  height=480  sample_aspect_ratio=4:3  display_aspect_ratio=16:9
```

The frame is still 640×480 and the file is still 15,349 bytes — identical
packets, correct shape.

## Options

- **Display aspect ratio** — the DAR to stamp on the file. Presets cover 16:9,
  9:16, 4:3, 3:4, 1:1, 21:9, 2.39:1, 1.85:1, 5:4, 4:5, 3:2, and 2:3. Choose
  `custom` to type your own.
- **Custom ratio** — used only when the ratio is set to `custom`. It accepts
  `16:9`, `16/9`, a plain decimal (`1.85` means width ÷ height), or display
  dimensions (`1920x1080`). Anything that works out between 0.05 and 20 is
  allowed; the value is reduced to an exact integer ratio before it is written,
  so `2.39` becomes `239:100`.
- **Output container** — `keep` rebuilds the same container as your input and is
  the safest lossless choice. `mp4`, `mkv`, `mov`, and `webm` remux into that
  container instead. This is stream copy, so the codecs must fit the container
  (H.264/H.265/AAC → mp4/mov/mkv; VP8/VP9/Opus → webm/mkv/mp4). `mkv` is the most
  tolerant target.
- **Web fast-start** — MP4/MOV only. Moves the index (`moov` atom) to the front of
  the file (`-movflags +faststart`) so players read the new ratio immediately and
  the clip streams progressively. Ignored for mkv/webm.

## Limits and edge cases

- Maximum input size is 64 MB, since everything is processed in your browser.
- Accepted inputs are the usual containers ffmpeg can remux: MP4, MOV, M4V, MKV,
  and WebM. Very exotic containers may need to be remuxed to `mkv` first.
- **This never changes pixels.** It cannot add letterbox bars, crop, or rescale.
  If you want bars or a real resize, use the aspect-pad, crop, or resize video
  tools instead — those re-encode by necessity.
- Quality settings do not apply here: there is no encoder in the pipeline, so
  there is no bitrate or CRF to choose.
- Matroska and WebM store display dimensions as whole pixels, so a ratio like
  16:9 can read back as an equivalent-but-not-identical fraction (for example
  `427:240`, which is 1.7792 against 16:9's 1.7778). MP4/MOV store the exact
  reduced ratio.
- Remuxing into a container the codec does not support (H.264 into `webm`, say)
  fails by design — use `keep` or `mkv`.
- Some old hardware players ignore the container tag entirely and always assume
  square pixels. Nothing done without re-encoding can fix those; you would have
  to rescale the pixels.

## FAQ

<details>
<summary>Does retagging the aspect ratio lose any quality?</summary>

No. The tool uses stream copy (`-c copy`), so the encoded audio and video packets
are copied unchanged into a new container. Nothing is decoded or re-encoded, and
the result is bit-for-bit identical apart from the corrected header. That is also
why it finishes almost instantly, even on long clips.

</details>

<details>
<summary>What is the difference between this and adding black bars or cropping?</summary>

This tool changes only **metadata** — the ratio the player is told to display. The
stored frame keeps exactly the same pixels and the same width and height. Adding
letterbox bars, cropping to a ratio, or stretching to new dimensions all rewrite
the actual picture, which means decoding and re-encoding. Use the aspect-pad,
crop, or resize video tools for those.

</details>

<details>
<summary>How do I reset a file to square pixels?</summary>

Choose `custom` and enter the video's **stored pixel size** as the ratio — for a
640×480 file, type `640x480`. That sets DAR equal to the frame's own shape, which
is the same as SAR 1:1. There is no ratio-free "make pixels square" switch,
because ffmpeg needs a concrete ratio when it is only copying streams.

</details>

<details>
<summary>My video shows 1.7792:1 instead of exactly 16:9 afterwards. Is that wrong?</summary>

No — that is a Matroska/WebM storage detail. Those containers record display
width and height as whole pixels rather than as a fraction, so the ratio is
rounded to the nearest whole-pixel pair. The visible difference is under a tenth
of a percent. Choose `mp4` or `mov` output if you need the exact reduced ratio in
the header.

</details>

<details>
<summary>Which ratio should I pick for social video?</summary>

`9:16` for vertical full-screen formats such as Reels, Shorts, and TikTok; `1:1`
for square feed posts; `4:5` for taller portrait feed posts; `16:9` for YouTube
and general landscape playback. If your source was shot anamorphic for cinema,
`2.39:1` (scope) and `1.85:1` (flat) are the standard theatrical ratios.

</details>

<details>
<summary>Can I multiply the current ratio instead of setting an absolute one?</summary>

Not directly — the tool builds its ffmpeg command without first probing the file,
so it has no way to read the existing ratio. If you know it, do the arithmetic
yourself and enter the result as a custom ratio: an existing 4:3 file that should
be 33% wider becomes `4:3 × 1.33`, i.e. roughly `1.78`, which you can enter as
`1.78` or `16/9`.

</details>
