# apply-alpha-mask competitor analysis (2026-07-26)

## Tool scope

`apply-alpha-mask` uses a second image as the transparency mask for a first image and returns a PNG with an 8-bit alpha channel. White mask pixels become opaque, black pixels become transparent, and gray pixels become partial transparency.

This is a chat + CLI tool in the current gizza model because it needs two ordered image inputs (`picture` first, `mask` second). The generic page surface in this repository supports a single upload for media tools, so a standalone generated page would be misleading until multi-file page inputs exist.

## Competitor scan

Search query used: `online image mask apply grayscale mask transparency alpha PNG`.

| Competitor / reference | Observed table-stakes | In-model decision |
| --- | --- | --- |
| Online PNG Tools alpha-mask utilities | PNG alpha workflows treat white as opaque, black as transparent, and gray as partial alpha; related tools expose alpha extraction/fill and mask visualization. | Implemented white/black/gray semantics and PNG output. Extraction/fill are separate operations and not included in this tool. |
| Small PNG Tools alpha-mask generator | Common controls include uploaded image input, background/color-derived mask generation, threshold, smoothing/soft edge, and PNG download. | Implemented threshold and smooth alpha preservation. Color/background automatic masking and smoothing filters are out of scope for a strict “apply this provided mask image” tool. |
| ImageOnline alpha-channel extractor / alpha viewers | Alpha masks are typically represented as grayscale images and downloaded for editing; workflows expect existing-alpha awareness. | Implemented channel selection including the mask image’s own alpha channel and an `existing_alpha` combine mode (`replace` or `multiply`). Extraction is a different one-image tool. |
| Desktop image editors (layer mask / matte workflows) | Layer-mask workflows commonly support invert, fit/transform the mask to the target layer, and preserving existing transparency. | Implemented `invert`, mask fit modes (`stretch`, `cover`, `contain`), and replace/multiply handling for the picture’s original alpha. |

## Table-stakes matrix

| Capability | Status | Notes |
| --- | --- | --- |
| Two image inputs: picture + mask | In-model, implemented | `images` is a required source list of exactly two image sources. |
| White = opaque, black = transparent, gray = partial alpha | In-model, implemented | Default channel is perceptual luminance. |
| Output PNG with alpha | In-model, implemented | PNG is always emitted because it preserves 8-bit transparency. |
| Invert mask | In-model, implemented | `invert=false` by default; `true` swaps light/dark semantics. |
| Choose mask channel | In-model, implemented | `luminance`, `average`, `red`, `green`, `blue`, `alpha`. |
| Fit/resize mismatched mask dimensions | In-model, implemented | `stretch` default; `cover` and `contain` preserve aspect ratio. |
| Hard threshold | In-model, implemented | `threshold=0` keeps smooth alpha; 1-255 binarizes. |
| Preserve existing transparency | In-model, implemented | `existing_alpha=replace` default; `multiply` intersects with source alpha. |
| Brush editing / painting a mask | Out of model | Requires an interactive canvas UI, not a pure block parameter surface. |
| Automatic background removal / subject segmentation | Out of model | Requires ML or heuristic color-key tooling; distinct from applying a supplied mask. |
| Feather/blur/smoothing slider | Deferred | Implementable with extra image filtering, but not required for the first strict mask-application version. |
| Standalone generated page | Out of current page model | Needs two upload controls; current generic page accepts one media upload. Chat and CLI are verified surfaces. |

## UX/control decisions

- `channel`, `fit`, and `existing_alpha` are enums so chat/CLI callers see fixed valid choices.
- `invert` is a boolean; the default follows common matte convention.
- `threshold` is an integer 0-255, with 0 meaning smooth/continuous alpha.
- The input description emphasizes ordering: first image is the picture whose RGB is kept; second image is the mask.

## Verification notes

The intended verification surface is core unit tests, canonical `target/block.wasm`, manifest sync from the live descriptor, CLI listing/argument parsing, and hygiene checks. A Playwright page spec is not applicable until gizza gains a multi-image page input for `Param::source_list` image tools.
