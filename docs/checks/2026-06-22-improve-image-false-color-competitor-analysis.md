# image-false-color — competitor analysis (2026-06-22)

## Tool
`image-false-color` maps a grayscale / thermal / depth / scientific image's per-pixel
luminance (Rec. 601 luma) through a scientific colormap to produce a false-colour heatmap
PNG. Pure-Rust (`image` crate), no ffmpeg / no model. Surfaces: **chat + CLI** (image-bytes
output → no standalone page, consistent with colorblind-simulator / image-color-quantize).

Params:
- `colormap` (enum, default `viridis`): viridis, magma, inferno, plasma, cividis, turbo, jet,
  hot, grayscale.
- `invert` (boolean, default false): flip the luminance (1 − L) before lookup so dark areas
  map to the high end.
- image via `url` ⊕ `ref`.

## Competitors surveyed
- **OpenCV `applyColorMap`** (docs.opencv.org) — the de-facto reference. Ships ~22 named
  colormaps (autumn, bone, jet, winter, rainbow, ocean, summer, spring, cool, hsv, pink, hot,
  parula, magma, inferno, plasma, viridis, cividis, twilight, turbo, deepgreen…). Programmatic,
  not an online tool.
- **conceptviz.app / vizcept.com Scientific Color Palette Generator** — palette/hex generators
  (build a discrete swatch set), colour-blind-friendly options. They generate *palettes*, not a
  recolour of an uploaded image — adjacent but a different job.
- **Misc GitHub "convert image to heatmap" gists / shadertoy jet comparisons** — apply jet /
  viridis to a grayscale image (the exact task this tool does), confirming the standard feature
  set: a handful of named colormaps + apply-to-image.

## Capability diff & gaps (fit-to-model)
| Capability | Competitors | image-false-color | Action |
|---|---|---|---|
| Perceptually-uniform maps (viridis/magma/inferno/plasma/cividis) | yes | yes (all 5) | covered — **added cividis** during improve |
| Legacy / vivid ramps (jet/rainbow, hot, turbo) | yes | yes (jet, hot, turbo) | covered |
| Plain grayscale ramp | sometimes | yes | covered |
| Invert / reverse colormap | yes (matplotlib `_r`) | yes (`invert`) | covered |
| Alpha preservation | varies | yes | covered |
| Long-tail OpenCV maps (bone, ocean, pink, parula, twilight, hsv, autumn…) | yes (~22 total) | no (9 curated) | **out-of-scope copy-bloat** — the 9 chosen span the perceptually-uniform + thermal + legacy-rainbow + grayscale space that covers the real use-cases; the rest are largely redundant/legacy. Documented, not built. |
| Per-channel / custom colour-stop maps | a few advanced tools | no | out of model for a simple enum tool; skip |

## Changes made during improve
- Added **`cividis`** (colour-blind-friendly perceptually-uniform map) — the one notable map
  every serious competitor ships that was missing. Control points sampled from matplotlib.
- All other major colormaps were already present.

## Verification
- `cargo test --workspace`: 7 tests pass (6 core incl. endpoint/invert/grayscale-identity/hot,
  1 drift-guard schema test).
- `wafer build`: chat `block.wasm` validates + instantiates (1374 KiB).
- CLI: `gizza tool image-false-color url=… colormap={hot,viridis,cividis} invert=true` all
  return valid PNGs; unknown colormap returns a clear error.
- No page surface (image-bytes output has no page render mode); not applicable, not claimed.

## Trademark / copy note
Colormap names (viridis, magma, jet, cividis, turbo…) are generic descriptive technical terms;
control-point RGB values are mathematical lookup tables (matplotlib / Google turbo are public).
No competitor copy, branding, or trademarks were copied.
