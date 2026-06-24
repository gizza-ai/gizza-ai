# color-contrast-checker — competitor analysis (2026-06-23)

New tool built from the backlog and improved against the top web-based WCAG
contrast checkers. All competitor notes are paraphrased — no copy, branding, or
trademarks were reproduced.

## Surfaces verified

- **Chat / API:** `cargo test --workspace` (21 tests incl. drift-guard schema
  test) + `wafer build` validates `target/block.wasm`.
- **CLI:** `gizza tool color-contrast-checker …` across text / json / suggest,
  hex / rgb / hsl / named-colour inputs, and error paths.
- **Page:** Playwright (4 specs) incl. the rgb→json path, the named-colour +
  suggest path, and a `?foreground=…&background=…` query-param deep-link.

## Competitors surveyed (top 5 + 1 bonus)

1. **WebAIM Contrast Checker** — hex (+ alpha) input, on-screen eyedropper;
   reports AA/AAA for normal + large text and AA for graphical/UI components;
   shows the ratio to 2dp; lightness sliders to nudge to passing (manual);
   shareable permalink and a JSON-via-URL API; live text preview.
2. **Adobe Color Contrast Analyzer** — hex/rgb/rgba, color-wheel and image
   extraction; AA/AAA text checks; live preview + colour-blindness simulation;
   palette/library oriented, no bare JSON/permalink.
3. **Coolors Contrast Checker** — hex with rgb/hsl swatch fields, eyedropper;
   AA/AAA small + large text; lightness sliders to raise the ratio; tied to the
   Coolors palette ecosystem.
4. **Stark** — multi-format dropdown (HEX/RGB/HSL/LAB/LCH/P3), mostly a design
   plugin on selected layers; AA/AAA; standout: suggested near-by passing swatches.
5. **Contrast Ratio (Lea Verou)** — any CSS notation incl. alpha; colour-coded
   zones; keyboard-nudge numbers; uniquely reports a ratio *range* for
   semi-transparent colours. (Original domain now 301-redirects to a Siege Media
   mirror; source still open on GitHub.)

Bonus — **Accessible Colors**: hex/rgb + font size/weight; AA/AAA; signature
feature is computing the closest passing colour by adjusting **lightness only**.

## Gap diff vs our initial build

| Gap | Dimension | Decision |
| --- | --- | --- |
| Full WCAG matrix (AA/AAA × normal/large + UI 3:1) | capability | **Already shipped** in the initial build |
| Machine-readable JSON output | capability | **Already shipped** (we do this better than most GUI tools) |
| HSL input | capability | **Built** — `hsl(h, s%, l%)` parser + rgb↔hsl conversion |
| CSS named-colour input | capability | **Built** — ~50 common names (incl. rebeccapurple) |
| Suggest a near-by accessible colour (Accessible Colors / Stark) | capability | **Built** — `format=suggest` + `target=aa\|aaa\|large`, lightness-only nudge keeping hue/saturation |
| 2dp ratio + readable report | copy/UX | **Already shipped** |
| Query-param deep-link (≈ WebAIM permalink) | UX | **Already shipped** via the page driver (`?foreground=…&background=…`) |
| Live text-on-background preview | UX/visual | **Out of model** — the shared page driver renders a single text/JSON output area, not a styled live preview canvas |
| Lightness sliders / swatch pickers / eyedropper | UX | **Out of model** — field-driven page; the suggest feature covers the same need programmatically |
| Alpha / semi-transparent colours + error-margin range | capability | **Considered, not built** — the two-colour opaque model is the common case; alpha compositing + a ratio range is a larger design change deferred for a focused follow-up |
| Wide-gamut LAB/LCH/P3 input | capability | **Out of model** — niche; sRGB hex/rgb/hsl/names cover the overwhelming majority of web use |
| Colour-blindness simulation, palette building | capability | **Out of scope** — separate gizza tools already cover colour-blind simulation; this tool stays a focused checker |

## Changes shipped in the improvement pass

- **Inputs:** added `hsl(...)` triples and ~50 CSS named colours alongside the
  original hex and rgb forms (`parse_color` now dispatches on all four).
- **New `suggest` format + `target` param:** returns the nearest accessible
  foreground (same hue/saturation, lightness nudged) that reaches AA (4.5:1),
  AAA (7:1), or large/UI (3:1); reports "no change needed" when it already passes
  and "change the background" when no shade of that hue can reach the target.
- **Schema drift-guard** regenerated for the new `format` enum value and `target`
  param; `manifest.json` kept consistent.
- **Page copy** (`content.md` / `meta.toml`) updated to document HSL/named inputs
  and the suggest feature; added a Playwright spec for both.

## Out-of-model features (considered, not built)

Live preview canvas, sliders/swatch pickers/eyedropper, alpha compositing with a
ratio range, wide-gamut LAB/LCH/P3, palette building, and colour-blindness
simulation — each needs either a richer page UI than the shared field-driven
driver provides or is better served by a separate tool. Listed here for the
record; not forced into this tool's model.
