# image-opacity — competitor analysis (2026-06-22)

Tool: set or scale the alpha/opacity channel of an image, returning a
semi-transparent PNG. Pure Rust (`image` crate). Surfaces: chat + CLI (image
bytes output → no page render mode, same as image-hsl-adjust / normalize-image).

## Verified surfaces

- **chat**: `wafer build` validates + instantiates `target/block.wasm` (the SW
  surface). Schema is single-sourced from `descriptor()`; drift-guard test
  `schema_json_matches_authored_chat_schema` passes.
- **CLI**: `gizza tool image-opacity url=… opacity=0.5 mode=scale` and
  `… opacity=0.25 mode=set` both produce a PNG (tested against a live QR PNG).
- **page**: N/A — image-bytes output has no page render mode (parity with
  image-hsl-adjust, normalize-image, image-grayscale).

## Top competitors surveyed (capability comparison only — no copy/branding reused)

1. **onlinepngtools.com "Make a PNG Transparent" / "Change PNG Opacity"** — set a
   uniform opacity on a PNG; slider 0–100%. Also offers color-keyed transparency
   (make one color transparent). Output PNG.
2. **redketchup.io / iloveimg-style opacity tools** — opacity percentage slider,
   live preview, PNG output, keeps existing alpha when reducing.
3. **picresize / resizepixel transparency** — set image transparency level
   (percent), download PNG.
4. **Photopea / GIMP "Layer → Opacity"** — multiply layer alpha by a factor
   (scale semantics) plus per-channel; full editor, out of scope.
5. **ImageMagick `-channel A -evaluate multiply 0.5` / `-alpha set`** — the two
   canonical primitives: multiply existing alpha (scale) vs. set a uniform alpha.

## Gap diff + ranking (fit-to-model)

| Capability | Competitors | image-opacity | Decision |
|---|---|---|---|
| Reduce opacity to a fraction | all | yes (`opacity` 0–1) | covered |
| Scale (multiply) existing alpha | Photopea/GIMP/ImageMagick | yes (`mode=scale`, default) | covered |
| Set uniform alpha regardless of source | onlinepngtools/ImageMagick `-alpha set` | yes (`mode=set`) | covered |
| Preserve existing transparency | redketchup/iloveimg | yes (scale mode) | covered |
| Accepts JPG/WebP/GIF/BMP input, always outputs PNG | most | yes (PNG out keeps alpha) | covered |
| Percent (0–100) vs fraction (0–1) input | onlinepngtools uses % | uses 0–1 fraction | intentional — matches gizza numeric-param convention; clamped + documented |
| Color-keyed transparency (make one color transparent) | onlinepngtools | no | OUT OF SCOPE — distinct tool (chroma-key / make-color-transparent); not opacity |
| Live preview slider | web UIs | N/A | gizza renders no page for image-bytes tools |
| Per-channel / gradient opacity | GIMP | no | out of scope (editor feature) |

## Improvements applied vs. a bare scaffold

- Two explicit modes (`scale` default + `set`) capture both canonical primitives
  (ImageMagick `evaluate multiply` vs `-alpha set`) instead of only one.
- `opacity` clamped to 0–1 and validated finite; out-of-range and NaN handled.
- `scale` mode preserves/attenuates existing transparency (matches the better
  competitors), while `set` gives a flat uniform alpha for badge/overlay use.
- Mode parser accepts synonyms (`multiply`→scale, `replace`/`uniform`→set) for
  forgiving LLM/CLI input.

No competitor copy, branding, or trademarks were used. Color-keyed transparency
is the only notable competitor feature left out, and it is a genuinely different
tool (chroma-key), not an opacity gap.
