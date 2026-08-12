# edge-detection competitor analysis (2026-08-12)

Backlog row: `edge-detection` — detects edges in an image using Canny or Sobel and returns the edge map.

## Competitor scan

Search query: "online edge detection Canny Sobel image tool thresholds blur invert output format".

Real tools reviewed from the search results:

| Competitor | Observed table-stakes | In gizza model? | Decision for this tool |
| --- | --- | --- | --- |
| Pixlane edge detection | Upload an image, choose among Canny/Sobel/Laplace/Scharr/Prewitt-style operators, export an edge visualization. | Mostly. Canny and Sobel map directly to ffmpeg filters; Laplace/Scharr/Prewitt are not first-class ffmpeg filters in the current page/runtime model. | Ship Canny and Sobel plus ffmpeg `edgedetect=mode=colormix` for a color overlay. List missing extra operators as out-of-model rather than faking them. |
| Edge Tools edge detector | File drop/browse/paste pattern, sample-file affordance, PNG/JPEG/WebP/GIF/BMP/AVIF input copy, Canny/Sobel/Laplacian choices. | Partly. The generic gizza page already gives file upload/drop and local browser processing; ffmpeg covers common browser image formats but AVIF support depends on the ffmpeg build. | Use the generic file input; document PNG/JPEG/WebP/GIF/BMP/TIFF inputs and first-frame handling for animated sources. |
| ImageTool edge detection | Positioning around Sobel, Canny and Laplacian edge detection for outline/contour output. | Partly. Sobel and Canny fit; Laplacian is omitted until there is a tested wasm-safe implementation/filter chain. | Include clear method labels and FAQs explaining Canny versus Sobel. |
| Toolexe edge detection | Threshold tuning, invert option, soften/blur before detection, PNG export, local browser processing. | Yes. Thresholds, invert, pre-blur and PNG/JPEG/WebP output are all simple ffmpeg argv parameters. | Implement sliders for low/high/blur, checkbox for invert, enum for format. Add presets for clean outline, fine detail, coloring page and inked photo. |
| Elysia image edge detect | Sobel/Prewitt/Laplacian/Canny choices, threshold and visualization controls. | Partly. Threshold controls fit Canny; visualization fit is color overlay and invert. Prewitt/Laplacian are deferred. | Ship the in-model visualization options: plain edge map, inverted line art, and color-mix overlay. |

## In-model feature set shipped

- Input: one image file/URL/ref, capped at 8 MB on the tool side.
- Methods: `canny` (default), `sobel`, `colormix`.
- Canny thresholds: `low` and `high` fractions from 0 to 1, with `high >= low` validation.
- Pre-blur: `blur` Gaussian sigma from 0 to 10 px.
- Output polarity: `invert=true` for black lines on white.
- Output formats: `png` default, `jpg`, `webp`.
- UX controls: enum selects for method/format, sliders for low/high/blur, checkbox for invert, preset chips.
- Worked examples: clean outline, fine detail, Sobel gradient, coloring page, inked photo.

## Out-of-model or deferred

- Laplacian, Scharr, Prewitt and Roberts operators are common in competitor UIs but are not shipped here because the current gizza/ffmpeg page model only has validated Canny/Sobel/edgedetect chains. They should be added only after a wasm-safe implementation or a verified ffmpeg-compatible filter chain is available.
- Batch processing, live camera input, vector export and segmentation-style edge cleanup are not part of the current single-file gizza block model.
- AVIF support is not promised because it depends on the browser ffmpeg build used by the generated page.

## Verification snapshot

- Core/block/web workspace tests passed with 17 total Rust tests.
- Canonical `scripts/build-block-wasm.sh edge-detection` produced `blocks/edge-detection/target/block.wasm` and `Cargo.lock`.
- `wasm-pack build blocks/edge-detection/web --target web --release --out-dir pkg` produced the browser package.
- `python3 scripts/sync-tool-manifest.py edge-detection` regenerated the descriptor-backed `manifest.json` and `wafer.toml` summary.
