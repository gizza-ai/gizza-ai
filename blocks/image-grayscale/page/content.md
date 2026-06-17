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
