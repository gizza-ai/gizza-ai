# image-bg-replace — competitor analysis (2026-07-06)

Tool: **image-bg-replace** — remove a photo's (solid/green-screen) background by
chroma-keying a target color, then composite the subject onto a new solid color, a
two-color gradient, or leave it transparent. Browser-local ffmpeg (`colorkey` +
`overlay`/`lutrgb`/`geq`), no upload, no account, no ML model.

## Scan method

One WebSearch ("green screen background remover chroma key online tool …") → skimmed the
top 3 real chroma-key competitors (paraphrased, no copy/branding reproduced):

1. **onlinepngtools.com — Remove PNG Chroma Key** (pure chroma-key)
2. **imageonline.io — Green Screen Remover** (YCbCr chroma-key + spill suppression)
3. **sleek.design — Green Screen Remover** (browser-local Canvas chroma-key)

(AI "one-click, no green screen" removers — remove.bg, Picsart, Kapwing, Evoto — were
also surveyed but their core is ML segmentation, which is out-of-model for a pure-Rust +
ffmpeg browser tool. See the out-of-model list.)

## Table-stakes matrix (each → descriptor param OR out-of-model)

| Table-stake (competitor) | Fit | Where it lands |
|---|---|---|
| Key-color selection (hex + green/blue presets) | in-model | `key_color` (color control), default `#00ff00`; preset chips |
| Similarity / tolerance threshold slider | in-model | `similarity` slider 0–100, default 30 → colorkey `similarity` |
| Edge softness / blend / feather slider | in-model | `blend` slider 0–100, default 10 → colorkey `blend` |
| Transparent PNG output (all 3 default to this) | in-model | `bg_type = transparent` (colorkey → PNG/WebP alpha) |
| Replace with a solid color | in-model | `bg_type = solid` + `bg_color`, drawn via `lutrgb` fill + `overlay` |
| Replace with a gradient | in-model | `bg_type = gradient` + `bg_color`/`bg_color2` + `direction`, via `geq` + `overlay` |
| Output PNG / WebP | in-model | `format = png\|webp\|jpg\|keep`, default png |
| Green/blue preset buttons | in-model | `[[example]]` preset chips |
| Blue-screen support | in-model | just set `key_color` (chip provided) |

### UX control patterns matched
- Sliders for the two numeric knobs (`similarity`, `blend`) → `kind = "slider"`.
- Color pickers for the three color fields → `kind = "color"` (hybrid swatch + hex text,
  still accepts named colors).
- Fixed-choice `bg_type`, `direction`, `format` → `Param::enumv` (`<select>`).
- Preset chips (green→white, blue→transparent, black→transparent, portrait fade) →
  `[[example]]`, the declarative answer to competitors' preset buttons.

## Out-of-model (listed, NOT built)

- **AI / automatic subject detection** (remove background with no green screen) — needs an
  ML segmentation model (U²-Net / rembg class); gizza is pure-Rust + ffmpeg, no model. The
  tool is explicitly a **chroma-key** replacer and the page says so.
- **Supplied-image background** — compositing over a second uploaded image needs a second
  file input; the page allows a single upload and chat-ffmpeg can't run multi-input in a
  Service Worker. Solid + gradient backgrounds are single-input filtergraphs and are built.
- **Eyedropper / click-to-pick key color from the image** — needs interactive canvas pixel
  sampling; the ffmpeg page marshals argv, not clicks. The color field + presets cover
  manual selection.
- **Spill suppression / de-fringe, clip-black, clip-white, matte-size** (imageonline) —
  advanced matte controls colorkey doesn't expose; `similarity`+`blend` cover the core
  keying quality. Considered, rejected to keep the schema honest to what ffmpeg delivers.
- **B&W removal-preview mode** (onlinepngtools) — a second render mode; considered,
  rejected (the live result already previews the composite).

## Decisions baked into the descriptor from the start

Params (order = meta.toml field order = `build_argv` order):
`key_color`, `similarity`, `blend`, `bg_type`, `bg_color`, `bg_color2`, `direction`,
`format`. Defaults: green key, similarity 30, blend 10, solid white background, gradient
white→black vertical, png output.
