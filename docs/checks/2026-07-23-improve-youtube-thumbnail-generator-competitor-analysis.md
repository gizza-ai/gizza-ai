# youtube-thumbnail-generator — competitor analysis (2026-07-23)

Pre-build competitor scan for `youtube-thumbnail-generator`. Findings are paraphrased;
no competitor copy, branding, or trademarks reproduced.

## Competitors scanned

1. Browser thumbnail makers for video creators: template canvas, upload/select a
   background image or frame, add large text, pick font/color/outline/shadow, export PNG/JPEG.
2. Online video-to-thumbnail generators: upload a video, choose a timestamp/frame,
   crop to 16:9, add simple text overlays, export a still image.
3. Social/thumbnail design editors: strong preset sizes (1280×720), accent shapes,
   brand colors, outline/shadow text, and preset chips/templates.

## Table-stakes params and model fit

| Table-stake | Decision |
| --- | --- |
| Video input and timestamp/frame selection | **in-model** → single `video/*` upload + ffmpeg `-ss` / `-frames:v 1` |
| 1280×720 YouTube-style canvas | **in-model** → width/height fields defaulting to 1280×720 |
| Crop/fit frame to canvas | **in-model** → ffmpeg `scale=...:force_original_aspect_ratio=increase,crop=...` |
| Large headline text | **in-model** → drawtext with bundled font + textfile |
| Text color and outline/shadow | **in-model** → validated color params + drawtext border/shadow |
| Accent bar / simple graphic element | **in-model** → drawbox on top/bottom/left/right |
| Preset chips/templates | **in-model** → page `[[example]]` presets |
| Drag-and-drop freeform layout | **out-of-model** — current gizza page controls are declarative fields, not a canvas editor |
| Background removal / AI subject cutout | **out-of-model** — needs segmentation/matting model |
| Stock images, stickers, template marketplace | **out-of-model** — requires external asset library and licensing |

## Design decisions

- Output is a PNG thumbnail, not a video.
- Input is a single video file, matching the current ffmpeg page model.
- The default canvas is 1280×720 with center crop, matching the common 16:9 thumbnail size.
- Headline text is sent to ffmpeg via `textfile=` and the font via `fontfile=` to avoid
  filtergraph injection and missing-font issues in ffmpeg.wasm.
- Accent bar is deliberately simple: top/bottom/left/right/none plus color and thickness.
- Text wrapping, drag handles, cutouts, sticker packs, and platform publishing are listed as
  out-of-model rather than silently omitted.
