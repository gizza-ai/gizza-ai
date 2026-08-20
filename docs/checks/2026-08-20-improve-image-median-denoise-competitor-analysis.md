# image-median-denoise competitor analysis — 2026-08-20

## Scope

New tool: `image-median-denoise`, an image denoise/despeckle page and CLI surface for removing salt-and-pepper noise, scanner dust, and small speckles with an edge-preserving median filter.

## Competitor scan (paraphrased)

Search query: online image median filter denoise salt and pepper noise tool adjustable radius channels output format.

| Competitor | Table-stakes observed | Fit for this tool |
| --- | --- | --- |
| BrushCue median filter | Upload-first image workflow; median filter described as per-channel median; small kernel-size range; immediate image preview/download. | In model. We expose a file upload, image preview/download, and a radius slider that maps to a median window. |
| ToolsJam image noise remover | Presents denoising as a task-oriented flow with a method choice, strength slider, and simple before/after mental model. | Partly in model. We build the median method directly and expose strength as radius/passes. Other denoise families are listed out of model for this focused block. |
| Classic image-editor despeckle/median controls (GIMP/ImageMagick-style patterns) | Radius/window size, repeat/recursive passes, options aimed at bright vs dark impulse noise, and warnings that high values flatten detail. | In model. Radius, passes, target bright/dark/both, and edge-case copy are included. |

## Parameter and UX decisions

| Need | Decision | Rationale |
| --- | --- | --- |
| Median strength | `radius` number/slider, 1-20, default 1 | Users think in window size/strength. Radius 1 gives the classic 3x3 salt-and-pepper filter; 20 is a practical upper cap. |
| Bright vs dark specks | `target` enum: `both`, `bright`, `dark` | Median percentile can lean toward darker or brighter neighbors for white dust or black pepper. |
| Channel targeting | `channels` enum: `all`, `luma`, `chroma` | Supports document cleanup, brightness noise, and high-ISO color blotches without adding a separate denoiser. |
| Recursive/despeckle passes | `passes` slider, 1-3, default 1 | A small repeated window is useful for dense impulse noise and remains simple to verify. |
| Output container | `format` enum: `keep`, `png`, `jpg`, `webp`; `quality` slider | Matches page-tool expectations: keep the source format by default, allow lossless scan output and smaller web output. |
| Privacy | `strip_metadata` checkbox | Common image-cleanup workflow before publishing. |
| Presets | Example chips for salt-and-pepper, scanner dust, white dust, chroma noise, heavy noise, and web JPG | Competitors use strength presets; chips make the generic page feel task-specific without custom JS. |

## Out-of-model or deliberately not built

- AI/non-local-means/bilateral denoisers: useful for Gaussian grain, but this block is a deterministic median filter and must run locally through the current ffmpeg/page model.
- Batch uploads and side-by-side editor UI: useful product features, but the current tool page model is a single-input transform with one output.
- Manual brush/mask cleanup: requires an image editor canvas workflow outside the generated control model.

## Verification intent

The page test covers actual image output by feeding a 9x9 white PNG with one black impulse pixel and asserting the resulting PNG is still 9x9 and the center pixel becomes white. It also checks query-param wiring, enum/select values, non-default checkbox state, output format conversion, and JPEG as a secondary input format.
