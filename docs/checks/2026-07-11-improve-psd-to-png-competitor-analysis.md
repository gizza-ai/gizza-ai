# psd-to-png competitor analysis (2026-07-11)

## Competitors scanned

| Competitor | Table-stakes behavior observed | UX/control patterns | Fit for gizza |
|---|---|---|---|
| Convertio PSD→PNG | Upload a PSD and receive a flattened PNG; emphasizes lossless output and browser-based conversion. | Single upload, target format implied by the route, download result. | In-model: flattened PNG from uploaded PSD. Out-of-model: server-side batch/account workflow. |
| CloudConvert PSD converter | Converts PSD to several image formats and exposes output controls such as resolution/quality/file size for image conversions. | Upload source, choose output format, quality/size controls where the output format supports them. | In-model: output format enum (`png`, `jpeg`) and JPEG quality. Out-of-model for this first block: broad many-format conversion matrix/resizing. |
| ConvertICO PSD→PNG | Advertises layer handling options: flatten all, visible-only, export each layer as separate files in a ZIP; background choices including transparent/white/black/custom. | Layer-mode select, background preset/color control, ZIP download for per-layer mode. | In-model: one flattened composite and configurable JPEG background. Out-of-model: per-layer multi-file ZIP output on current single-result gizza surface. |
| CoolUtils PSD→PNG | Flattens visible layers, preserves transparent background when possible, delivers a viewable PNG. | File upload, optional conversion settings on a hosted page. | In-model: flattened PNG that preserves alpha. |
| psdtopng.com | Client-side-flavored PSD parsing, transparent background, individual layer extraction claims. | Upload, convert/download, layer extraction as a separate download mode. | In-model: local PSD parsing and flattened PNG. Out-of-model: individual layers as multiple files. |

## Decisions for this tool

Built the highest-value in-model baseline: decode a Photoshop `.psd` locally and render its stored flattened composite to a single downloadable/viewable image. The block supports:

- `format=png|jpeg` enum. PNG is the default and preserves alpha; JPEG is useful for broad compatibility and smaller output.
- `quality=1..100` for JPEG, default 90, matching common image-converter controls.
- `background=#rgb|#rrggbb` for JPEG flattening because JPEG cannot represent transparency.
- Explicit error messages for non-PSD input and unsupported/invalid arguments.

Out-of-model or deferred:

- Exporting each layer as separate PNGs needs a multi-file/ZIP output surface. Current gizza tool calls return one result envelope, so advertising layer extraction would be misleading.
- Full Photoshop feature parity (adjustment layers, smart objects, color-management transforms, text/vector re-rendering) is outside this parser-based model. The tool uses the PSD's stored composite preview, which is the reliable one-image handoff path.
- Broad target-format conversion (WebP/TIFF/AVIF) is better served by chaining the output through existing image converters or by later adding more enum values if the page/chat surfaces can verify them.

## Verification notes

The implementation uses the pure-Rust `psd` crate for parsing and the already-proven `image` crate for PNG/JPEG encoding. Tests generate a minimal PSD fixture in memory, avoiding copyrighted binary fixtures while still exercising real PSD parsing and image encoding.
