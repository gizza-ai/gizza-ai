## About this tool

Use this when an editor, finishing system, or interchange hand-off asks for DNxHD/DNxHR media instead of a small delivery MP4. The output is encoded with ffmpeg's `dnxhd` encoder as DNxHR and wrapped as either QuickTime `.mov` or OP1a `.mxf`, so it is easier for NLEs to scrub, trim, grade, and re-export than long-GOP delivery codecs.

A typical default run takes a camera or phone clip, chooses **SQ — default standard-quality edit tier**, keeps **Container** at **MOV**, leaves **Resolution cap** at **Source size**, and writes 16-bit PCM audio. For example, a short MP4 test clip becomes a `.mov` whose video stream uses the DNxHR SQ profile, `yuv422p`, and an editor-friendly intra-frame structure.

DNxHR files are intentionally large. Choose **LB** plus a **720p** cap for offline/proxy work, **HQX** for 10-bit grading, or **444** when a 4:4:4 VFX/keying round trip matters. The page runs locally in the browser; the CLI/chat path caps input at 32 MiB and output at 160 MiB to avoid surprise memory use.

## Limits and edge cases

- This tool exposes DNxHR profiles (`dnxhr_lb`, `dnxhr_sq`, `dnxhr_hq`, `dnxhr_hqx`, `dnxhr_444`). Classic fixed-raster DNxHD bitrate modes are deliberately not exposed because ffmpeg rejects them unless the source exactly matches a DNxHD raster/frame-rate table.
- DNxHR profile and pixel format are linked. Leave **Pixel format** on **Auto** unless you need a validation check; explicit formats are accepted only when they match the selected profile.
- **Resolution cap** only downscales. Selecting 1080p for a 720p source leaves it at 720p instead of inventing pixels.
- The ffmpeg DNx encoder rejects rasters below 256×120. The tool reports that encoder failure rather than silently upscaling or padding your video.
- Browser previews usually cannot play DNxHR or MXF directly. Download the result and inspect it in an editor, media inspector, or `ffprobe`.

## FAQ

<details>
<summary>Which DNxHR profile should I pick?</summary>

Use **SQ** for everyday online editing, **LB** for small offline/proxy intermediates, **HQ** for higher-quality 8-bit 4:2:2, **HQX** for 10-bit grading, and **444** when 4:4:4 chroma is needed for compositing or keying.

</details>

<details>
<summary>Should I choose MOV or MXF?</summary>

Choose **MOV** when you want the broadest desktop compatibility. Choose **MXF** when a broadcast/Avid-style interchange workflow expects OP1a MXF files. Both wrappers contain the same DNxHR video essence selected by the profile.

</details>

<details>
<summary>Why does the pixel format field reject some combinations?</summary>

DNxHR profiles dictate their pixel format: LB/SQ/HQ use `yuv422p`, HQX uses `yuv422p10le`, and 444 uses `yuv444p10le`. The tool validates that pairing before ffmpeg runs so you get a clear error instead of an opaque encoder failure.

</details>

<details>
<summary>Does converting to DNxHR improve source quality?</summary>

No. DNxHR preserves quality well through editing and repeated exports, but it cannot restore detail, dynamic range, or color information that was not present in the source. Use it before editing or finishing, not as a quality enhancer.

</details>
