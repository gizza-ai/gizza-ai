## About this tool

Use this aspect ratio validator when an image, video, thumbnail, ad creative or screen recording must match a delivery spec such as `16:9`, `9:16`, `4:5`, `1:1` or `2.39:1`. Enter the measured width and height, set the target ratio, and the tool reports a PASS/FAIL verdict with the exact percentage deviation.

The report includes the reduced ratio, decimal ratio, orientation, nearest standard ratio, and practical repair dimensions. If the frame is too wide or too tall, it suggests both the largest crop that fits inside the current frame and the smallest padded canvas that contains it. Enable even dimensions when the result will go through video encoders that reject odd pixel sizes.

Everything is pure arithmetic and runs locally in your browser. To validate a real image file, first use an image metadata tool to read the width and height, then paste those dimensions here.

## Worked examples

```bash
gizza tool aspect-ratio-validator width=1920 height=1080 target=16:9 tolerance_percent=0
```

Returns `PASS` because 1920×1080 reduces exactly to `16:9`.

```bash
gizza tool aspect-ratio-validator width=1600 height=1200 target=16:9 tolerance_percent=1 even_dimensions=true
```

Returns `FAIL` because 4:3 is too tall for a 16:9 target, and includes crop/pad dimensions that can be copied into an editor or ffmpeg filter.

## Limits and edge cases

- Width and height must be positive numbers up to 1,000,000.
- `target` accepts `16:9`, `16/9`, `1920x1080`, `1.85:1` or a decimal such as `1.7778`.
- `tolerance_percent=0` means exact ratio only; the default `1` allows small rounding differences.
- `orientation_agnostic=true` lets a rotated frame satisfy the target, for example 1080×1920 can pass a 16:9 spec.
- Omit `target` to get an informational ratio report without a PASS/FAIL verdict.

## FAQ

<details>
<summary>Does this inspect an uploaded image file?</summary>

No. This tool validates dimensions you already know. Use an image metadata or media-info tool first to read the pixel width and height from a file, then paste those numbers into this validator. Keeping this block numeric makes it fast, private and deterministic.

</details>

<details>
<summary>What tolerance should I use for delivery specs?</summary>

Use `0` when the destination requires an exact ratio. Use the default `1` percent when you want to allow tiny rounding differences such as a one-pixel height mismatch. Larger tolerances are useful for quick audits, but they can hide real crop problems.

</details>

<details>
<summary>What is the difference between crop and pad suggestions?</summary>

Crop keeps the output inside the current frame and discards edge pixels until the target ratio is reached. Pad keeps every original pixel and expands the canvas with blank space. The report gives both, so you can choose the right fix for thumbnails, videos or print assets.

</details>

<details>
<summary>Why is there an even-dimensions option?</summary>

Many video codecs and delivery systems prefer or require even pixel dimensions. When `even_dimensions=true`, suggested crop and pad sizes are rounded to even integers so they are safer for H.264/H.265 workflows.

</details>
