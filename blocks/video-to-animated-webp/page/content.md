## About this tool

Convert a video clip into a looping **animated WebP**. Animated WebP is a modern GIF alternative: it usually produces much smaller files, keeps smoother color, and can preserve transparency when the source has alpha.

Everything runs locally in your browser through ffmpeg WebAssembly. The video is not uploaded.

## Controls

- **Start** trims from a point in the video, in seconds.
- **Duration** limits the clip length. Leave it at `0` to encode from the start point to the end, but short clips are strongly recommended.
- **Frames per second** controls smoothness and size. The default is 12 fps; 8–15 fps is a good range for sharing.
- **Width** scales the output while keeping the aspect ratio. Height is rounded to an even value for codec compatibility.
- **Quality** controls lossy WebP quality from 0 to 100; the default is 80.
- **Lossless WebP** preserves detail and transparency without lossy quantization, but can be much larger.

## Worked example

For a one-second preview from the start of a 128 px video, use:

```text
start = 0
duration = 1
fps = 8
width = 96
quality = 75
lossless = false
```

The generated command uses ffmpeg's `libwebp` encoder, loops forever (`-loop 0`), drops audio, and writes `out.webp`.

## Tips

- Keep the clip short. Animated WebP is efficient, but long high-fps clips are still large.
- Downscale before raising compression. A 320 px WebP often looks better and weighs less than a full-size one at very low quality.
- Use lossy mode for camera/video content and lossless mode for transparent UI animations, stickers, and screen captures.
- If a platform does not animate WebP, convert the same source to GIF with the Video to GIF tool.

## Limits and edge cases

- Input videos are capped at **25 MiB** and outputs at **25 MiB**.
- `fps` accepts 0–60; 0 means the default 12 fps.
- `width` accepts 0–4096 px; 0 keeps source width.
- `quality` accepts 0–100; 0 means default 80 in lossy mode.
- Audio is intentionally removed.
- Browser support for animated WebP is broad in modern browsers, but older apps may still prefer GIF.

## FAQ

<details>
<summary>Why use animated WebP instead of GIF?</summary>

Animated WebP usually gives smaller files and better color than GIF because it is not limited to a 256-color palette. It can also carry alpha transparency, which makes it useful for stickers, UI captures, and short transparent motion graphics.

</details>

<details>
<summary>Will it loop?</summary>

Yes. The output is encoded with `-loop 0`, which tells WebP players to loop forever. That matches the default behavior people expect from GIF-style animations.

</details>

<details>
<summary>What quality should I choose?</summary>

Start with 75–85 for video content. Lower values make smaller files with more artifacts; higher values keep more detail. For transparent graphics or sharp UI, try **Lossless WebP** instead.

</details>

<details>
<summary>Can I keep the audio?</summary>

No. Animated WebP is an image animation format, like GIF, and has no audio track. If you need sound, export MP4 or WebM instead.

</details>
