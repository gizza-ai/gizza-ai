## Convert Images to Grayscale Instantly

Remove color from any image directly in your browser — no software to install, no files
sent to a server. Your image stays on your device the entire time.

## How It Works

Upload a JPEG, PNG, WebP, or other image format. The tool uses ffmpeg (compiled to
WebAssembly) to apply the `format=gray` filter, stripping all color information and
producing a true grayscale output in the same format as the input.

## Why Grayscale?

Grayscale images are smaller, work well for printing, and are commonly used in design
workflows, document scanning, and artistic effects. Converting to grayscale is also a
first step in many computer-vision preprocessing pipelines.

## Supported Formats

JPEG, PNG, WebP, GIF, BMP, and most other common image formats supported by ffmpeg.
The output keeps the same file format as the input.

## FAQ

<details>
<summary>Does the converted image keep my original format?</summary>

Yes. The tool applies ffmpeg's `format=gray` filter and writes the result using the
same extension as the input — a PNG comes back as a PNG, a JPEG as a JPEG. The
downloaded file gets a `-gray` suffix, so `photo.jpg` becomes `photo-gray.jpg`.

</details>

<details>
<summary>Is there a maximum image size?</summary>

Yes — the input image is capped at 4 MiB, and the grayscale output must also fit
within 4 MiB. Larger files are rejected before conversion starts. If your image is
over the limit, resize or compress it first.

</details>

<details>
<summary>Will a transparent PNG keep its transparency?</summary>

No. The `gray` pixel format has no alpha channel, so transparent areas are flattened
during conversion. If you need grayscale with transparency preserved, keep a copy of
the original alpha mask to re-apply in an image editor.

</details>

<details>
<summary>Is my photo uploaded anywhere during conversion?</summary>

No — ffmpeg runs as WebAssembly inside your browser tab, so the pixels never leave
your device. Closing the page discards everything.

</details>
