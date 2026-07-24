## Social image resizer

Upload one image, choose a social-media preset, and get a resized/cropped image
with the right dimensions for that placement. The conversion runs in your browser
through ffmpeg WebAssembly, so the image is not uploaded.

### What it supports

- **Targets** — Instagram square, portrait, story/reel; Facebook post and cover;
  X/Twitter post and header; LinkedIn post and cover; YouTube thumbnail;
  Pinterest pin; and TikTok vertical video.
- **Fit modes** — **Cover** fills the target and crops overflow, **Contain** fits
  the whole image and pads the empty area, and **Stretch** forces the exact size
  even if that changes the aspect ratio.
- **Crop gravity** — when using cover crop, choose center, top, or bottom to keep
  the important part of the image.
- **Padding colour** — for contain mode, use a colour picker or a hex value such
  as `#ffffff` or `#111827`.

### Example

For a landscape product shot that needs to become an Instagram Story:

1. Upload the image.
2. Choose **Instagram story/reel — 1080×1920**.
3. Keep **Cover** to fill the whole vertical frame, or choose **Contain** with a
   white background to preserve the full original image.
4. Download the resulting 1080×1920 image.

### Limits & edge cases

- This tool emits **one output image at a time**. Many social tools advertise a
  download-all ZIP; the current gizza page model produces a single file, so use
  the preset chips to re-run the same source for each target you need.
- Output keeps the input format. A PNG input returns PNG, JPEG returns JPEG, and
  so on as supported by ffmpeg in the browser.
- Cover crop does not expose a manual drag crop canvas. Use top/center/bottom
  gravity for common framing adjustments.
- Very large source images are limited by browser memory and the ffmpeg runtime.

### FAQ

<details>
<summary>Does this upload my image?</summary>

No. The resize runs locally in your browser with WebAssembly. The selected file
is not sent to a server by this tool.

</details>

<details>
<summary>Which fit mode should I choose?</summary>

Use **Cover** for social posts that must fill the entire frame with no bars. Use
**Contain** when you must preserve the whole source image and are okay with
padding. Use **Stretch** only when exact dimensions matter more than preserving
the original aspect ratio.

</details>

<details>
<summary>Can it export every platform size at once?</summary>

No. It generates one file for the selected target size. The preset chips make it
quick to switch targets and re-run, but multi-file ZIP export is outside the
current single-output tool model.

</details>

<details>
<summary>Can I pick a custom crop rectangle?</summary>

Not interactively. Choose top, center, or bottom crop gravity for cover mode. If
you need pixel-perfect manual cropping, crop the image first and then run this
preset resize.

</details>
