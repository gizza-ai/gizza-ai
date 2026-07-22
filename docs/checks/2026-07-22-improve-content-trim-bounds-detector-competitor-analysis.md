# content-trim-bounds-detector — competitor analysis (2026-07-22)

Paraphrased notes only — no competitor copy, branding, or trademarks reproduced.

## Function

Given an image, find the tight bounding box of the "real" content (everything that is
not the uniform background / not fully transparent) and report the crop that would remove
the surrounding empty margin — **without** producing a cropped image. It measures; the
sibling `image-trim` tool actually crops.

## Top 3 real tools scanned

1. **Pillow `Image.getbbox()`** (Python imaging library). Returns the bounding box
   `(left, upper, right, lower)` of the non-zero / non-fully-transparent region, or `None`
   for an empty image. `getbbox(alpha_only=True)` keys on the alpha channel. It has no
   built-in color tolerance — you first difference against a background color (or invert)
   to get a mask, then call `getbbox`. Common uses: auto-crop, content detection, layout
   analysis, file-size trimming.

2. **Pico Image "Trim" (online)**. Scans pixels from each edge toward the centre. Offers a
   **Transparent** mode (reads the alpha channel) and a **Background Color** mode (compares
   edge pixels against a reference color within a **tolerance**). Aims for a tight box and
   consistent framing across photos with uneven margins.

3. **ImageMagick `-trim` (+ `-format "%@"`)**. `-trim` detects the border color from the
   image corners and removes the uniform border; `-format "%@"` prints the trim bounding box
   as `WxH+X+Y` (dimensions + offset) instead of cropping. `-fuzz N%` is the color tolerance;
   `+repage` resets the page offset. Corner-color auto-detection is the default convention.

## Table-stakes → in-model / out-of-model decisions

| Capability | Competitor(s) | Decision |
|---|---|---|
| Transparent (alpha) background detection | Pillow, Pico | **in-model** — `mode=transparent`; auto samples corners |
| Solid-color background, auto-detected from corners | ImageMagick, Pico | **in-model** — `mode=auto` corner vote |
| Explicit background color | Pico, ImageMagick (fuzz vs a color) | **in-model** — `color` param (`#rgb`/`#rrggbb`) |
| Color tolerance / fuzz | Pico, ImageMagick | **in-model** — `tolerance` 0–255 |
| Report bbox as offset + dimensions (`WxH+X+Y`) | ImageMagick `%@`, Pillow | **in-model** — `content_x/y/width/height` |
| Report per-side trim margins | (derived from bbox) | **in-model** — `trim_left/top/right/bottom` |
| Safety padding / keep-margin around content | common crop UX | **in-model** — `padding` 0–500 |
| Tolerate noisy borders (a few stray pixels) | (robustness) | **in-model** — `background_percent` 50–100 |
| Empty-image → "no content" answer | Pillow returns `None` | **in-model** — `has_content=false`, note |
| Actually output the cropped image | Pico, ImageMagick, Pillow | **out-of-model here** — this tool only measures; point users to `image-trim` |
| Draw an overlay/preview of the box on the image | Pico, BBox overlay tools | **out-of-model** — needs image output; a detector returns data (no page) |
| Batch / folder processing | desktop tools | **out-of-model** — no server/batch; one image per call |
| Interactive marquee drawing | annotation tools | **out-of-model** — this is automatic detection, not manual |

## UX / surface

Image-input analyzers in this repo (image-info, image-color-picker, document-skew-detector,
image-horizon-tilt-checker) are **chat + CLI only, no standalone page** — the page file-input
path is ffmpeg-only and this is a pure-Rust decoder that returns a JSON report. So this tool
ships as a chat + CLI JSON report, matching that family. All in-model table-stakes land in the
descriptor; the crop-the-image capability is delegated to `image-trim` and named in the output
note so the two tools compose.

Sources:
- [Pillow Image.getbbox()](https://www.codecademy.com/resources/docs/pillow/image/getbbox)
- [Pico Image Trim](https://picoimage.com/trim)
- [Crop borders / whitespace (ImageMagick)](https://www.baeldung.com/linux/image-crop-borders-white-spaces)
