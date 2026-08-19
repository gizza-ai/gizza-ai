# color-transfer — competitor analysis (2026-08-16)

Scan run before implementation per create-next-tool. Query: `color transfer image match reference color statistics online`. Observations are paraphrased only.

## Competitors reviewed

| # | Tool | Table-stakes observed |
|---|------|-----------------------|
| 1 | Browser color-transfer demo using Reinhard color transfer | Takes a target/source image and a reference image, keeps target geometry, uses Lab mean/std matching, exposes transfer strength, and previews/downloads the recolored result. |
| 2 | Python/OpenCV color transfer recipes and notebooks | Common options are Lab statistics, RGB statistics, histogram matching, preserving luminance, and JPEG/PNG output. Most warn that the method copies a color mood, not objects or lighting. |
| 3 | Photo color-match / LUT-style online tools | Offer stronger histogram-style matching, saturation/intensity controls, before/after comparison, and export quality/format choices. |

## Decisions

| Capability | Decision |
|---|---|
| Two image inputs: target first, reference second | In model — `images` is a required `source_list` of exactly 2 image sources. |
| Keep target size and alpha | In model — output canvas follows the target image; the reference is sampled only for statistics; alpha is preserved for PNG. |
| Lab mean/std (Reinhard-style) transfer | In model — `method=lab-stats` default. |
| RGB mean/std and histogram alternatives | In model — `method=rgb-stats` and `method=histogram`. |
| Gentle color-cast-only mode | In model — `method=mean-only` shifts means without changing target contrast. |
| Blend/intensity control | In model — `strength` 0–100 blends the recolored pixels over the original. |
| Preserve original luminance | In model — `preserve_luminance` keeps target L and gamut-maps by desaturating instead of clipping. |
| Saturation control | In model — `saturation` 0–200 after transfer. |
| PNG/JPEG output and JPEG quality | In model — `format=png|jpeg`, `quality=1..100`. |
| Interactive before/after slider | Out of model — image-byte output tools do not have a generic standalone page render mode. |
| Full LUT export / batch folders / accounts | Out of model — this repo ships local blocks only, not multi-file projects or cloud workflows. |

## Surface decision

This is a multi-image-in, image-bytes-out tool, matching existing `image-composite`, `image-collage`, and `gif-from-images` patterns. It ships as chat + CLI with no standalone page. The page generator cannot currently render an arbitrary image-byte output with two image uploads for this pattern, so no Playwright page spec is applicable.

## Duplicate check

Existing image tools cover resizing, format conversion, compositing, alpha masks, and color adjustments, but none takes a reference image and transfers its color statistics onto a target. `image-composite` combines pixels geometrically; `levels-adjust` and related tools apply explicit user controls to one image. `color-transfer` is therefore not a semantic duplicate.
