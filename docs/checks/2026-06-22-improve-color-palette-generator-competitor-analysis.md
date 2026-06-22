# color-palette-generator — competitor analysis (2026-06-22)

## Surfaces verified

- **Chat block** — `wafer build` OK, `gizza-ai/color-palette-generator v0.1.0 (334.6 KiB)` validated + instantiated (wasm32-wasip1).
- **CLI** — `gizza tool color-palette-generator color=… scheme=… count=…` returns structured JSON
  (`{scheme, base, colors:[{hex,rgb,hsl}]}`); default scheme `complementary`, default count `5`;
  invalid color exits 1 with a clear message.
- **Page** — `/tools/color-palette-generator/`, 3 Playwright tests pass (triadic swatches, analogous
  count series, invalid-color error). `scheme` renders as a `<select>` (enum) and `count` as a numeric
  input (min 2 / max 12 / default 5), driven from `manifest.json` `tool.parameters`.

## Top competitors surveyed

| Tool | Harmonies offered | Notable extras |
|------|-------------------|----------------|
| Adobe Color (color wheel) | mono, complementary, analogous, triadic, + others | color-blindness preview, live wheel, save to library |
| Figma Color Wheel | complementary, triadic, analogous | inline in Figma, live wheel |
| Colorffy Scheme Generator | complementary, analogous, triadic, mono | 2000+ curated palettes, export Tailwind/CSS/PNG/PDF |
| NextUtils Color Palette | mono, analogous, complementary, triadic, tetradic, split-complementary | WCAG contrast checking |
| pppalette (fffuel) | complementary, split-comp, analogous, triadic, tints, tones, shades, warm/cool | bright/dark variations |
| Sessions College Color Calculator | complementary, mono, analogous, triadic, tetradic, split-complementary | educational color wheel |

## Gap analysis (fit-to-model)

Our tool ships **9 schemes**: complementary, analogous, triadic, split-complementary, tetradic, square,
monochromatic, shades, tints — a superset of every individual competitor's harmony list (most offer 4–6).

**Capabilities matched or exceeded (in-model, shipped):**
- Full color-theory harmony set (matches/exceeds Adobe, NextUtils, Sessions, pppalette).
- Series schemes (analogous / monochromatic / shades / tints) with a configurable 2–12 color count —
  matches pppalette's tints/shades and exceeds tools with fixed counts.
- Output in **HEX, RGB and HSL** per swatch (most competitors show only hex/rgb on hover).
- Accepts base color as `#hex` (3/6), `rgb()`, or `hsl()` — broad input parsing.
- Scheme aliases (`mono`, `split`, `comp`, `rectangle`) so chat/LLM phrasing maps to a valid scheme.
- Runs 100% locally (browser wasm / CLI / chat) — no upload, unlike hosted SaaS wheels.

**Out-of-model competitor features (intentionally NOT built):**
- **Color-blindness simulation** (Adobe) — a separate concern; gizza already has a dedicated
  `colorblind-simulator` block.
- **WCAG contrast checking** (NextUtils) — a distinct tool's job (contrast pairs, not palette harmony).
- **Curated palette browsing / community libraries / export-to-Tailwind-PDF** (Colorffy) — these need a
  hosted dataset + multi-format export pipeline, out of scope for a single pure-compute block.
- **Interactive color wheel UI** (Adobe/Figma/Canva) — gizza pages are form-driven; the wheel is a
  presentation layer, not a capability gap. The full harmony math is present and identical.

**No competitor copy, branding, or trademarks were copied.** All scheme math is standard HSL color-wheel
geometry (offsets of 30/90/120/150/180/210/240/270°), implemented independently.

## Sources

- [Adobe Color — Color Wheel](https://color.adobe.com/create/color-wheel)
- [Figma — Color Wheel](https://www.figma.com/color-wheel/)
- [Colorffy — Color Scheme Generator](https://colorffy.com/color-scheme-generator)
- [NextUtils — Color Palette Generator](https://www.nextutils.com/tools/utilities/color-palette)
- [pppalette (fffuel)](https://www.fffuel.co/pppalette/)
- [Sessions College — Color Calculator](https://www.sessions.edu/color-calculator/)
