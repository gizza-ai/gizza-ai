## Shrink a photo for sharing in one step

Phone photos are huge — a modern camera shot can be 5–12 MB, more than messaging
apps, email, and web forms want to carry, and every one of them still holds the
GPS coordinates and camera details baked in by your phone. This tool does the
three things you actually want before sharing, in a single pass:

1. **Downscale** the longest side to a size that still looks sharp on a phone or
   in a browser (default 1600 px), keeping the aspect ratio and never upscaling.
2. **Strip the metadata** — EXIF, GPS location, timestamps and comments — so you
   are not leaking where and when the photo was taken.
3. **Compress** by re-encoding at your chosen quality, optionally converting to a
   lighter format (JPEG or WebP shrink photos the most).

Everything runs locally in your browser with WebAssembly ffmpeg. Nothing is
uploaded to a server.

### Worked example

Take a 4032 × 3024 JPEG straight off a phone (about 4.8 MB, with GPS EXIF).
With **Max size = 1600**, **Quality = 80**, **Format = JPEG**, and **Strip
metadata = on**, the result is a 1600 × 1200 JPEG of roughly 250–400 KB with no
EXIF or GPS — a ~90% smaller file that is still crisp for chat or a web upload.
Set **Max size = 0** to keep the original dimensions and only compress + strip.

### Presets

The chips under the inputs are one-click starting points: **Messaging** (1600 px
JPEG), **Story / vertical** (1080 px WebP), **Email friendly** (1024 px JPEG),
and **Just compress** (keep the original size, quality 70). Adjust any field
afterwards.

## Limits and edge cases

- **Supported formats:** JPG/JPEG, PNG and WebP for both input and output. HEIC
  and other formats are not decoded here — convert them first.
- **Longest-side cap:** `Max size` caps the *longer* of width/height; the shorter
  side scales proportionally. A smaller image is never enlarged. Output
  dimensions are rounded to even numbers so JPEG always encodes cleanly.
- **PNG quality:** PNG is lossless, so on PNG the quality slider only changes how
  hard the encoder works (higher effort → smaller file), not the pixels. To make
  a photo-heavy PNG much smaller, convert it to JPEG or WebP.
- **Transparency + JPEG:** JPEG has no alpha channel, so converting a transparent
  PNG to JPEG fills the transparent areas with black. Keep PNG or choose WebP to
  preserve transparency.
- **Orientation:** stripping metadata removes the EXIF orientation flag. Photos
  whose rotation is already baked into the pixels are unaffected; if a phone photo
  comes out rotated, re-save it with orientation applied first.
- **True "compress to exactly 200 KB":** not offered — hitting an exact byte
  target needs an iterative quality search. Use the quality and max-size sliders
  to trade size against sharpness.

## FAQ

<details>
<summary>How is this different from the resize or compress tools?</summary>

The `image-resize` tool only changes dimensions, `image-compress` only re-encodes
at a quality, and `strip-exif` only removes metadata. This tool does all three at
once with sharing-friendly defaults, so you don't have to chain three steps to get
a photo ready for a message or upload.

</details>

<details>
<summary>Does it upload my photo anywhere?</summary>

No. The image is processed entirely in your browser with WebAssembly ffmpeg. The
file never leaves your device, which is also why stripping GPS/EXIF here is a real
privacy win rather than trusting a server.

</details>

<details>
<summary>What size should I pick for messaging apps?</summary>

**1600 px** on the longest side (the default) is a good all-round choice — most
chat apps display images around that size, so anything larger is wasted bytes. For
a status/story use **1080 px**, and for an email attachment **1024 px** keeps the
file comfortably small.

</details>

<details>
<summary>Should I convert to JPEG or WebP?</summary>

For photos, both **JPEG** and **WebP** are much smaller than PNG; WebP is usually
a bit smaller again at the same quality and is supported by all modern browsers and
apps. Keep **PNG** for flat graphics, logos or screenshots with sharp edges or
transparency. Choose **keep** to leave the format unchanged.

</details>

<details>
<summary>Why did my PNG barely get smaller?</summary>

PNG is lossless, so lowering the quality only asks the encoder to work harder — it
can't throw away detail the way JPEG/WebP do. A photographic PNG will shrink far
more if you set the format to **JPEG** or **WebP**.

</details>
