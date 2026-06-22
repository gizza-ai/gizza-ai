# image-to-sketch — competitor analysis (2026-06-22)

Tool: `gizza-ai/image-to-sketch` — turn a photo into a pencil or line-art sketch.
Surfaces: chat + CLI (pure-Rust `image` crate; no standalone page — image-bytes
output has no page render mode, same as `image-pixelate-censor` / `add-text-to-image`).

## Capability

Pure-Rust, no ffmpeg, no model. Two looks, single `strength` knob:

- **pencil** — the classic sketch pipeline (grayscale → invert → gaussian blur →
  color-dodge blend), the exact "negate + blur + color-dodge" recipe the
  reference tools describe. Soft graphite shading on a near-white ground.
  `strength` = blur radius/softness (default 8).
- **lineart** — inverted Sobel gradient edge map: clean black outlines on white,
  the "contour / coloring-page" look. `strength` = edge sensitivity, higher =
  darker/thicker lines (default 1).

Input via `url` (HTTP/HTTPS) or `ref`; output is a grayscale PNG. Decodes
PNG/JPEG/WebP/GIF/BMP.

## Competitors surveyed (top 5)

| Tool | Styles offered | Intensity control | Notes |
|------|----------------|-------------------|-------|
| imageonline.co (Pencil sketch) | pencil sketch | pencil shadow + thickness sliders | classic negate/blur/color-dodge pipeline, in-browser |
| Canva — Photo to Sketch | pencil, line art, ink | preset-based | part of the Canva editor |
| imagetosketch.com | pencil sketch, line drawing | basic | upload → one-click |
| Fotor — Photo to Sketch | pencil, line, ink, charcoal | intensity slider | AI-assisted |
| VanceAI / VansPortrait | pencil, line drawing (AI) | preset | AI model, line-drawing focus |

Common feature set across the field: (a) a soft **pencil/graphite** mode built on
the negate→blur→color-dodge recipe, (b) a clean **line-art / contour** mode, and
(c) an **intensity/strength** control. No signup, runs in-browser, files
auto-deleted.

## Gap analysis vs. our tool (fit-to-model)

**Covered (in-model, shipped):**
- Pencil/graphite mode using the standard color-dodge pipeline — matches the core
  capability every competitor centers on.
- Line-art / contour mode — matches the "line drawing / coloring-page" preset.
- Intensity control via `strength` (pencil softness / line sensitivity), with a
  sensible default when 0 — matches the slider competitors expose.
- Wide input format support + URL or ref input; deterministic, private (runs
  locally on every backend incl. the chat Service Worker — no upload to a 3rd party).

**Out of model (intentionally NOT built — would need a model or are stylistic
embellishments):**
- AI-stylized / "authentic graphite stroke" rendering (Fotor/VanceAI use ML
  models — gizza is pure-Rust + ffmpeg, no model).
- Charcoal / cross-hatch / colored-pencil / anime-outline preset *styles* beyond
  pencil + lineart. These are aesthetic variants on top of the same edge/shade
  math; left out to keep the schema tight. Could be added later as extra `mode`
  values without new deps.
- Paper-grain / texture overlay (decorative compositing, not a core capability).
- Colored output (we emit grayscale, which is the canonical sketch look).

**Conclusion:** the tool covers the full *in-model* capability set the market
offers (pencil shading, line art, adjustable strength) with a clean two-mode
schema. The only deltas are ML-stylization (out of model) and additional preset
styles (cosmetic, deferrable). No capability/copy/UX gap left open within model.

## Verification

- `cargo test --workspace` — 6 tests pass (5 core: parse_mode, pencil dims/decode,
  pencil light background, lineart dark-edge, error path; 1 block: chat-schema
  drift guard).
- `wafer build` — block.wasm validates/instantiates (1401 KiB).
- CLI — `gizza tool image-to-sketch url=… mode=pencil` and `…mode=lineart
  strength=2` both produce valid grayscale PNGs; bad `mode` returns a clear error.
- No page (image-bytes output) → no Playwright surface, by design.

## Sources

- [imageonline.co — Pencil sketch](https://pencilsketch.imageonline.co/)
- [Canva — Photo to Sketch](https://www.canva.com/features/photo-to-sketch/)
- [imagetosketch.com](https://imagetosketch.com/)
- [Fotor — Photo to Sketch](https://www.fotor.com/features/photo-to-sketch/)
- [VanceAI / VansPortrait — Photo to Sketch](https://vanceai.com/photo-to-sketch/)
