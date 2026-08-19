# image-blank-detector competitor analysis (2026-08-12)

## Scope

`image-blank-detector` identifies failed image renders and exports: all-white, all-black, solid-colour, fully transparent, and near-blank frames with only tiny stray marks. The gizza model is pure Rust + wasm block execution; it can inspect decoded pixels from an image `url` or prior `ref`, but it does not provide a browser upload page for image-byte analyzers in this repo.

## Competitor scan

Scanned table-stakes behavior from common image QA and batch-processing tools: ImageMagick `identify`/histogram workflows, FFmpeg `blackdetect`/`blackframe` style thresholds for frame emptiness, OpenCV blank-image examples, and online “blank/solid image” checkers used for QA triage.

| Table-stakes capability | Typical competitor pattern | Gizza decision |
| --- | --- | --- |
| All-white / all-black detection | Report whether every pixel is near white or near black, often with a tolerance | In model: `verdict` values `all_white` and `all_black`, plus `dominant_hex`, luma range, and coverage. |
| Solid-colour detection | Histogram dominant-colour check or unique-colour count | In model: dominant fill clustering, `solid_color` verdict, `unique_colors`, `channel_range`, entropy, mean colour. |
| Transparency handling | Treat alpha-only canvases as empty; optionally inspect raw alpha/RGBA | In model: `ignore_transparency` default true, `transparent` verdict, transparent percent, warning when raw transparent RGB is compared. |
| Tolerance for compression noise | Accept a threshold because JPEG/WebP near-uniform files are not byte-identical | In model: `tolerance` numeric parameter, default 2% channel distance. |
| Near-blank threshold | Flag frames that are empty except for a watermark, cursor, crop mark, or stray glyph | In model: `blank_threshold` numeric parameter, default 99.5% dominant-colour coverage; `near_blank` verdict. |
| Evidence for auditability | Histogram/entropy/dominant colour statistics rather than a bare yes/no | In model: returns confidence, coverage, unique colours, luma min/max/mean/stddev, entropy, dominant and mean hex. |
| Batch-friendly summary | One-line output suitable for logs | In model: `note` starts with BLANK / NOT BLANK and includes key deciding numbers. |
| Visual diff / mask overlay | Some desktop QA tools show where non-background pixels are | Out of model here: current gizza block returns JSON/text evidence only; no generated overlay asset is emitted. |
| Browser drag-and-drop upload UI | Online checkers use a local file picker and preview | Out of model for this repo pattern: image-byte analyzers expose chat/CLI via URL/ref; the generic pure-tool page cannot pass uploaded image bytes to the block. |

## Descriptor defaults and UX decisions

- `tolerance = 2.0`: absorbs a few channel levels of JPEG/WebP compression wobble without allowing visibly different content to merge into the background.
- `blank_threshold = 99.5`: catches a blank page with a tiny watermark or stray glyph while leaving ordinary icons/screenshots as not blank.
- `ignore_transparency = true`: treats fully transparent pixels as one empty colour so hidden RGB under alpha does not create false detail.
- Input uses `Input::Image` (`url` or `ref`) to match existing image analyzers such as `image-info` and `image-average-color`.

## Worked examples

- A 1000x1000 white PNG with every pixel `#ffffff` returns `is_blank: true`, `verdict: "all_white"`, `coverage_percent: 100.0`, `entropy: 0.0`.
- A transparent PNG canvas returns `verdict: "transparent"` and includes the transparent-pixel percentage.
- A mostly white export with a 20x20 watermark on a 1000x1000 canvas returns `verdict: "near_blank"` at the default threshold.
- A half-black/half-white image returns `is_blank: false`; the dominant colour covers only half the frame and luma spans the full range.

## Limits and edge cases

- The block decodes common raster formats supported by the Rust `image` crate configuration (PNG, JPEG, WebP, GIF, BMP).
- Animated images are decoded as an image by the decoder; this tool is for static render/export triage, not temporal video blank detection.
- Very large images are refused before allocating more than the sandbox budget.
- Entropy is supporting evidence, not the verdict by itself: a two-colour image has low entropy but can still be real content.
