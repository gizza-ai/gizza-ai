# image-histogram-analyzer — competitor analysis & decisions (2026-08-17)

**Tool:** `gizza-ai/image-histogram-analyzer` — decode an image and report exact RGB plus luminance histograms with exposure, clipping, dynamic-range, contrast and colour-cast statistics. Chat + CLI surface (image input + text/JSON report); no standalone generated page because this is an `Input::Image` byte-decoder, matching the existing no-page pattern for image-info-style file tools.

## Competitor scan

Search query: `online image histogram analyzer RGB luminance clipping exposure stats tool`.

1. **Scanly Image Histogram** — browser-side upload/paste/sample flow. Table stakes observed: RGB and luminance channel histograms; channel toggles; exposure, dynamic range, contrast, shadow clipping, highlight clipping and analysis details; per-channel mean/median/std-dev; image dimensions/file format/file size; histogram copy/download; hints such as log-scale viewing for sparse bins.
2. **CocoShot Image Histogram Viewer** — drag/drop file selector with separate luminance, red, green and blue channel views. Table stakes observed: total pixels and per-channel means, a human explanation of histogram reading, and a simple upload-and-analyse workflow.
3. **ImageMagixOnline Histogram Visualizer** — drop/paste/upload UI with demo presets. Table stakes observed: RGB plus weighted luminance histograms, 0-255 binning, brightness/exposure educational copy, preset images for balanced/low-key/high-key/high-contrast cases, mode/statistics explanations, and SVG/PNG chart export.
4. **Imagic Tools Free Image Histogram Analyzer** — upload UI with red/green/blue display toggles, luminance analysis type, grid option, image statistics and downloadable analysis. Table stakes observed: JPG/PNG/GIF/WebP support, max input size, channel display controls, exposure/color-distribution summary, and clipping-at-ends explanation.

## Requirements mapped to the gizza model

| Competitor capability | Decision | Implementation |
| --- | --- | --- |
| RGB channel histograms | In-model | `histogram.red`, `histogram.green`, `histogram.blue` report binned counts; per-channel stats include min/max/mean/median/std-dev/percentiles/mode/distinct levels/clipped-end counts. |
| Luminance/brightness histogram | In-model | `luma` parameter supports `rec601`, `rec709`, `average`, and `max`; `histogram.luma` and `luma` stats are returned. |
| Exposure and clipping verdicts | In-model | `exposure`, `shadow_clipped`, `highlight_clipped`, `contrast`, `reason`, and `note` summarize the analysis with the thresholds that drove it. |
| Adjustable bin count | In-model | `bins` accepts 2-256; stats stay full precision while the reported arrays are folded. |
| Clipping threshold controls | In-model | `clip_margin` and `clip_percent` make exact-end or near-end clipping explicit and reproducible. |
| Transparent-pixel handling | In-model | `ignore_transparent` defaults true, reports transparent count/percent, and can be disabled to measure stored RGB under alpha. |
| Compact/full/export output modes | In-model | `output=summary` for compact JSON, `output=full` for arrays, `output=csv` for spreadsheet-ready rows. |
| Local/private processing | In-model | Pure Rust image decoding in wasm/CLI; input is resolved through the standard image source path rather than sent to a site-specific backend. |
| Chart display, channel toggles, log-scale chart controls | Out-of-model for this repo surface | The public toolkit exposes structured JSON/CSV; the private consuming UI may render charts later. The core returns enough data for that UI. |
| SVG/PNG histogram chart download | Out-of-model for this block | Chart rendering is deliberately not included; `csv`/`full` export the data needed to draw one. |
| Drag/drop/paste/browser upload page | Out-of-model here | Existing generator cannot pass uploaded bytes to this pure image decoder; this block follows the no-page Input::Image pattern used by image-info. |
| EXIF/camera metadata | Out-of-scope | This tool analyses decoded pixel tones only; metadata is a separate tool class. |

## Worked examples covered by verification

- A generated black/white PNG reports both-end clipping, high contrast, balanced mean luma and neutral cast.
- `output=full` with four bins returns four-channel arrays with the expected counts.
- `output=csv` emits one CSV row per bin with `bin,level_start,level_end,red,green,blue,luma` columns.
- Non-default `luma`, `clip_margin`, `clip_percent`, `ignore_transparent`, and output modes are covered by unit/CLI checks.

## Honest limits

- This analyser does not modify the source image and does not equalize, stretch or repair tones.
- It counts decoded 8-bit-per-channel pixels; HDR/raw camera pipeline interpretation is not attempted.
- GIF input is analysed after image-crate decoding rather than as a multi-frame timeline.
- Large images are rejected before they can exceed the wasm sandbox memory budget.
