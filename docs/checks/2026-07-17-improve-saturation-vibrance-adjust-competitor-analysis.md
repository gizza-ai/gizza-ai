# saturation-vibrance-adjust — competitor analysis (2026-07-17)

Tool function: selectively adjust image colour intensity. `saturation` is a flat global saturation scale; `vibrance` is a nonlinear selective adjustment that boosts muted pixels more than already-saturated pixels and can protect skin-tone hues. This is distinct from `image-hsl-adjust`, which only applies a uniform HSL saturation factor.

## Competitors scanned

Paraphrased from common photo editors and online image-adjustment tools found for “vibrance saturation image adjust online”:

1. Browser photo editors with Saturation/Vibrance controls — expose separate sliders for saturation and vibrance; vibrance is described as affecting muted colours more gently than already vivid areas.
2. Raw/photo editors — include vibrance as a “natural colour boost” control and keep global saturation as the stronger flat adjustment.
3. Online colour-enhancement tools — provide simple sliders, preserve image dimensions, and export a PNG/JPEG result; most do not expose implementation details but table-stakes UX is numeric slider input.

No competitor copy or branding is reused here.

## Table-stakes params → decision

| Capability | In/out model | Decision |
|---|---|---|
| Global saturation | in-model | `saturation` in -1..1, where -1 is grayscale and +1 doubles saturation. |
| Selective vibrance | in-model | `vibrance` in -1..1; positive changes scale with remaining saturation headroom, so muted colours move more than vivid pixels. |
| Skin-tone protection | in-model | `protect_skin` default true damps vibrance around orange/skin hues at normal lightness. |
| Preserve alpha/dimensions | in-model | Output PNG preserves alpha and dimensions. |
| Live preview/sliders | out-of-surface for this image-bytes tool | Descriptor exposes numeric/boolean controls to chat+CLI. Pure image-byte tools in this repo do not have a browser page renderer. |
| JPEG/WebP export | out-of-scope | Single output is PNG to preserve alpha and keep the image-output contract simple. |
| AI auto-enhance | out-of-model | Would require a learned model; not built. |

## Verification plan

The block is verified with core unit tests that prove vibrance boosts muted colours more than vivid colours, skin protection reduces the vibrance gain, saturation=-1 grays out pixels, alpha is preserved, bad input errors, and the chat schema drift guard matches the descriptor. CLI checks decode output PNG dimensions for default, vibrance, saturation, and `protect_skin=false` runs.
