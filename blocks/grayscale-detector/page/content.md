## What this tool does

Paste image bytes as **base64** or **hex** and the detector checks whether the decoded image is effectively grayscale. It scans every visible pixel, compares the RGB channels, and reports how many pixels still carry color information even if the image looks black-and-white.

Use it before converting RGB assets to a single-channel format, auditing print or archival images, or checking whether compression added tiny channel differences. The output includes dimensions, scanned pixel counts, gray/color percentages, max and mean score, and optional sample coordinates with hex values for the first color pixels.

## How grayscale detection works

A pixel is counted as gray when its colorfulness score is less than or equal to the **tolerance**.

- **Channel delta** (default) scores `max(R,G,B) - min(R,G,B)`. Use tolerance `0` for strict `R=G=B`, or `2` to absorb tiny JPEG/WebP noise.
- **Saturation** scores HSV saturation on a 0-255 scale. This is stricter for dark tinted pixels where the raw channel delta is small but the hue is visible.

The detector supports PNG, JPEG, WebP, GIF, BMP, and TIFF inputs that the browser and Rust image decoder can read. Decoded input is capped at **32 MiB**. `max_samples` is capped at **200** listed color pixels; set it to `0` when you only need counts.

## Worked example

For a 2×2 PNG containing three gray pixels and one green-tinted pixel, strict channel-delta mode reports:

```text
Status: contains color pixels
Dimensions: 2×2 (4 pixels)
Metric: RGB channel delta (max - min of R, G, B)
Tolerance: 2
Scanned pixels: 4
Gray pixels: 3 (75.0000%)
Color pixels: 1 (25.0000%)
Max RGB channel delta: 10
Mean RGB channel delta: 3.0000
Sample color pixels: (1,0) #1e281e rgb(30,40,30) score 10
Suggestion: keep RGB/color storage, or convert deliberately before saving as grayscale.
```

## Limits and edge cases

- Alpha is ignored by default, so transparent pixels are still scored by their RGB channels. Turn **Ignore alpha channel** off to exclude fully transparent pixels from the verdict.
- Animated formats are decoded as the frame provided by the underlying image decoder; do not use this as an animation-wide audit.
- Color profiles are not transformed. The detector evaluates decoded RGB channel values, not perceptual appearance after color-management.
- A tolerance of `255` accepts every scanned pixel and is useful only as a boundary or wiring check.

## FAQ

<details>
<summary>What tolerance should I use?</summary>

Use `0` when you need a strict byte-level check that every pixel has identical R, G, and B channels. Use `1` or `2` when checking JPEG/WebP exports where compression can introduce tiny channel differences that are visually gray.

</details>

<details>
<summary>Why are there two metrics?</summary>

Channel delta is easy to interpret and works well for storage decisions. Saturation is stricter for dark tinted pixels: a pixel such as `rgb(12,0,0)` has a small channel delta but full HSV saturation, so saturation mode flags it as colored.

</details>

<details>
<summary>Does alpha affect the result?</summary>

By default alpha is ignored and every pixel is scored by RGB. If you set **Ignore alpha channel** to false, fully transparent pixels are skipped and reported separately because they do not contribute visible color.

</details>

<details>
<summary>Can this convert the image to grayscale?</summary>

No. This tool audits whether the input is already effectively grayscale. If it reports color pixels, convert the image intentionally in an image editor or pipeline, then run the detector again to confirm the result.

</details>
